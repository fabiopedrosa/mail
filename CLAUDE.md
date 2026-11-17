# CLAUDE.md

## Builds run in GitHub Actions — don't build locally

Do not run `cargo build`, `cargo check`, `cargo test`, or other full compiles on
this machine. A local build of this project takes well over 10 minutes and
blocks the session. Builds and checks happen in GitHub Actions instead:

- `.github/workflows/quality.yml` — runs on every push to `main` and on pull
  requests (checks, lints, tests).
- `.github/workflows/native-platforms.yml` — packages the app. Run it manually
  with `gh workflow run native-platforms.yml` (defaults to Windows x64 only in
  this fork).

To verify a change: commit, push, then follow the run with `gh run list` /
`gh run watch` and read failures with `gh run view --log-failed`.

## Message open timings

Every message open appends one JSON line to
`%LOCALAPPDATA%\flectar-mail\logs\open-timings.jsonl` (rotates to
`open-timings.1.jsonl` past 4 MB; `FLECTAR_OPEN_TIMINGS=0` turns it off).
`phases_ms` holds milliseconds since the click for `load_requested`,
`body_downloading`, `loaded`, `prepared`, `first_frame` and `resources_ready`.
See `src/open_timings.rs`.
