//! How long opening a message takes, phase by phase.
//!
//! Every open appends one JSON line to `open-timings.jsonl` in the app's cache
//! directory under `logs` (`%LOCALAPPDATA%\flectar-mail\logs` on Windows), so
//! slow opens can be studied later. Times are milliseconds since the click:
//!
//! - `load_requested`: the conversation read was queued for Tokio.
//! - `body_downloading`: the read found the body still on the server.
//! - `loaded`: the read returned the body to the UI thread.
//! - `prepared`: the HTML was parsed, styled and laid out.
//! - `first_frame`: the first frame of the real body reached the reader.
//! - `resources_ready`: no image or font of the message is still loading.
//!
//! A line is written when every resource has settled, or with
//! `"outcome": "superseded"` when another open starts first. Only the UI
//! thread records phases. `FLECTAR_OPEN_TIMINGS=0` turns the log off.

use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    io::Write,
    path::PathBuf,
    sync::{OnceLock, mpsc},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Older lines move to `open-timings.1.jsonl` past this size.
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

struct Open {
    started: Instant,
    started_unix_ms: u128,
    conversation: i32,
    trigger: &'static str,
    phases: Map<String, Value>,
    details: Map<String, Value>,
}

impl Open {
    fn has(&self, phase: &str) -> bool {
        self.phases.contains_key(phase)
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Open>> = const { RefCell::new(None) };
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        !std::env::var("FLECTAR_OPEN_TIMINGS")
            .is_ok_and(|value| matches!(value.as_str(), "0" | "false" | "no" | "off"))
    })
}

/// A user opened conversation `conversation` from the list (`"list"`) or
/// another message of the open thread (`"thread"`).
pub(crate) fn start(conversation: i32, trigger: &'static str) {
    if !enabled() {
        return;
    }
    finish("superseded");
    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    CURRENT.with(|current| {
        *current.borrow_mut() = Some(Open {
            started: Instant::now(),
            started_unix_ms,
            conversation,
            trigger,
            phases: Map::new(),
            details: Map::new(),
        });
    });
}

/// Record `phase` the first time it happens for the current open.
pub(crate) fn mark(phase: &'static str) {
    with_open(|open| record(open, phase));
}

/// Attach a detail, such as the HTML size, to the current open.
pub(crate) fn note(key: &'static str, value: impl Into<Value>) {
    let value = value.into();
    with_open(|open| {
        open.details.insert(key.into(), value);
    });
}

/// The body for the open message reached the UI thread. A body still on the
/// server is downloaded first and read again.
pub(crate) fn loaded(body_downloading: bool) {
    with_open(|open| {
        if body_downloading {
            record(open, "body_downloading");
        } else {
            record(open, "loaded");
        }
    });
}

/// The real body, not the snippet shown while it loads, was laid out. Bodies
/// that need no read count as loaded.
pub(crate) fn prepared(message_id: i32, html_bytes: usize, gpu: bool) {
    with_open(|open| {
        if open.has("load_requested") && !open.has("loaded") {
            return;
        }
        record(open, "prepared");
        open.details.insert("message_id".into(), message_id.into());
        open.details.insert("html_bytes".into(), html_bytes.into());
        open.details
            .insert("renderer".into(), if gpu { "gpu" } else { "cpu" }.into());
    });
}

/// The message reopened with the body already laid out; nothing was redrawn.
pub(crate) fn unchanged() {
    let done = with_open(|open| {
        if open.has("load_requested") && !open.has("loaded") {
            return false;
        }
        record(open, "prepared");
        open.details.insert("unchanged".into(), true.into());
        true
    });
    if done == Some(true) {
        finish("complete");
    }
}

/// A frame reached the reader. Frames of the snippet before `prepared` don't
/// count.
pub(crate) fn frame() {
    with_open(|open| {
        if open.has("prepared") {
            record(open, "first_frame");
        }
    });
}

/// The renderer gave up and showed plain text instead.
pub(crate) fn failed(error: &str) {
    if with_open(|open| {
        open.details.insert("error".into(), error.into());
    })
    .is_some()
    {
        finish("failed");
    }
}

/// How the message's images and fonts stand after a frame or a resource poll.
/// The open is complete once a frame is shown and none is still loading.
pub(crate) fn resources(loading: usize, ready: usize, failed: usize, blocked: usize) {
    let complete = with_open(|open| {
        if !open.has("first_frame") {
            return false;
        }
        open.details.insert(
            "resources".into(),
            json!({ "loading": loading, "ready": ready, "failed": failed, "blocked": blocked }),
        );
        if loading > 0 {
            return false;
        }
        record(open, "resources_ready");
        true
    });
    if complete == Some(true) {
        finish("complete");
    }
}

fn with_open<T>(f: impl FnOnce(&mut Open) -> T) -> Option<T> {
    if !enabled() {
        return None;
    }
    CURRENT.with(|current| current.borrow_mut().as_mut().map(f))
}

fn record(open: &mut Open, phase: &'static str) {
    if !open.has(phase) {
        let elapsed = open.started.elapsed().as_secs_f64() * 1000.0;
        open.phases
            .insert(phase.into(), json!((elapsed * 10.0).round() / 10.0));
    }
}

fn finish(outcome: &'static str) {
    let Some(open) = CURRENT.with(|current| current.borrow_mut().take()) else {
        return;
    };
    let mut line = Map::new();
    line.insert("at_unix_ms".into(), json!(open.started_unix_ms as u64));
    line.insert("version".into(), env!("CARGO_PKG_VERSION").into());
    line.insert("conversation".into(), open.conversation.into());
    line.insert("trigger".into(), open.trigger.into());
    line.insert("outcome".into(), outcome.into());
    line.insert("phases_ms".into(), Value::Object(open.phases));
    line.extend(open.details);
    write_line(Value::Object(line).to_string());
}

/// Lines go to a writer thread so the UI never waits on the disk.
fn write_line(line: String) {
    static WRITER: OnceLock<Option<mpsc::Sender<String>>> = OnceLock::new();
    let writer = WRITER.get_or_init(|| {
        let path = log_path()?;
        let (sender, receiver) = mpsc::channel::<String>();
        std::thread::Builder::new()
            .name("open-timings".into())
            .spawn(move || {
                for line in receiver {
                    let _ = append(&path, &line);
                }
            })
            .ok()?;
        Some(sender)
    });
    if let Some(writer) = writer {
        let _ = writer.send(line);
    }
}

fn append(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("1.jsonl"));
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

#[cfg(desktop)]
fn log_path() -> Option<PathBuf> {
    let paths = flectar_mail_core::config::Paths::default_dirs().ok()?;
    Some(paths.cache_dir.join("logs").join("open-timings.jsonl"))
}

#[cfg(not(desktop))]
fn log_path() -> Option<PathBuf> {
    None
}
