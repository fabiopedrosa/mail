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
