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

## Stay rebased on upstream

This repo is a fork of `flectar/mail` (remote `upstream`). Rebase `main` onto
`upstream/main` whenever possible — before starting new work and whenever
upstream has moved:

```
git fetch upstream
git rebase upstream/main
git push --force-with-lease origin main
```

Every rebase ends with a summary for the user of what upstream brought in
(`git log --oneline <old-base>..upstream/main`): grouped by theme (features,
fixes, Gmail/sync, rendering, l10n, CI…), one line each in plain words, with
upstream PR numbers where the merge commits name them. Call out anything that
touches code our fork changed, and any fork commit dropped because upstream
now covers it.

Resolve conflicts in favour of keeping our fork's changes on top. If a fork
commit is now covered upstream, drop it. Push and let `quality.yml` verify the
result.

## Message open timings

Every message open appends one JSON line to
`%LOCALAPPDATA%\flectar-mail\logs\open-timings.jsonl` (rotates to
`open-timings.1.jsonl` past 4 MB; `FLECTAR_OPEN_TIMINGS=0` turns it off).
`phases_ms` holds milliseconds since the click for `load_requested`,
`body_downloading`, `loaded`, `prepared`, `first_frame` and `resources_ready`.
See `src/open_timings.rs`.
