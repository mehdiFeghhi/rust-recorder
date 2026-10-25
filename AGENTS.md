# AGENTS.md

Operating instructions for AI coding agents working in this repository. Read
this file before changing anything, then read
[CONTRIBUTING.md](CONTRIBUTING.md).

## Project state

This project is a Linux screen recorder written in Rust.

Development is organized into small, reviewable milestones:

- **Phase 0 — Engineering foundation:** repository configuration, CI/CD,
  dependency governance, documentation, and development tooling. *(merged)*
- **Phase 1, M1 — Configuration and CLI:** argument parsing, configuration
  validation, help output, and predictable exit codes. *(merged)*
- **Phase 1, M2 — FFmpeg command construction:** build and test FFmpeg
  arguments without starting the process. *(merged)*
- **Phase 1, M3 — Recording process lifecycle:** start, monitor, and stop
  FFmpeg safely. *(merged)*
- **Phase 2 — Hardening and v0.1.0:** observability, output probing,
  integration tests, and release. *(merged)*
- **Phase 3 — Capture configuration:** rectangular regions, monitor discovery
  and selection, and interactive selection. *(merged)*
- **Phase 4 — Audio:** source discovery, selection, and recording. *(current:
  M4.1 audio design)*
- **Later milestones:** GUI development and Wayland support.

The current milestone must be confirmed from the task instructions and
repository state before implementation begins.

The application targets Linux. The initial capture backend uses FFmpeg with X11
screen capture. Do not assume Wayland support.

## Hard rules

1. **Stay in scope.** Implement only the milestone explicitly approved by the
   user. Do not implement later milestones preemptively.
2. **No unrequested dependencies.** Every new crate must be justified in the
   PR: the problem it solves, alternatives considered, license, MSRV, and
   maintenance status. If a new license is introduced, update `deny.toml`
   deliberately.
3. **Never weaken quality gates.** Do not add blanket `#[allow(...)]`,
   `continue-on-error`, `--no-verify`, or relax `-D warnings` to make a check
   pass. Fix the underlying problem.
4. **Git operations require authorization.** Do not commit, push, merge, tag,
   or publish unless explicitly authorized by the user.
5. **Stay inside the repository.** Do not modify files outside this repository
   unless the user explicitly authorizes it.
6. **Protect sensitive data.** Never log or embed captured screen contents,
   audio contents, credentials, or tokens. Follow
   [docs/observability.md](docs/observability.md).
7. **Keep `main` releasable.** Changes must be formatted, lint-clean, tested,
   and reviewed before merging.
8. **Preserve user work.** Inspect `git status` before editing. Never reset
   history, discard user changes, or overwrite existing work.
9. **Do not claim unverified behavior.** Clearly distinguish automated test
   results, manual test results, and unverified behavior. Compilation alone
   does not prove that screen recording works.
10. **Prefer safe Rust.** Do not introduce `unsafe` without prior discussion
    and explicit approval.

## Milestone boundaries

### M1 — Configuration and CLI (merged)

Allowed:
- Argument parsing.
- Configuration types and defaults.
- Input validation.
- Help output.
- User-facing configuration errors.
- Tests for parsing and validation.

Not allowed:
- Starting FFmpeg.
- Capturing the screen.
- Recording audio.
- GUI implementation.

### M2 — FFmpeg command construction (merged)

Allowed:
- Building an FFmpeg command from validated configuration.
- Constructing arguments using `std::process::Command`.
- Unit-testing executable arguments and options.
- Validating command configuration and output settings.

Not allowed:
- Spawning FFmpeg.
- Starting an actual recording.
- GUI or audio implementation.

Command construction must not use shell-string interpolation. Pass executable
arguments separately to avoid shell interpretation.

### M3 — Recording process lifecycle (merged)

Allowed:
- Starting FFmpeg.
- Managing the child process.
- Handling startup and process failures.
- Implementing graceful shutdown.
- Waiting for process termination and checking exit status.
- Reporting recording errors.

Requirements:
- Never overwrite an existing recording silently.
- Handle child-process cleanup deliberately.
- Do not claim success unless FFmpeg exits successfully and the expected
  output has been checked appropriately.
- Keep process management separate from CLI parsing where useful.
- M3.1 adds synchronous execution and exit-status reporting; M3.2 adds
  Ctrl-C handling and graceful shutdown; M3.3 verifies that the output file is
  a non-empty regular file after a successful exit.

Audio capture and GUI development remain out of scope unless separately
approved.

### Phase 2 — Hardening (merged)

Allowed:
- Structured logging with `tracing`.
- Output verification with `ffprobe`.
- Integration tests with a fake FFmpeg executable.
- Documentation and release preparation; version tags only with explicit
  approval.

Not allowed:
- GUI, audio, Wayland, or streaming unless separately approved.

### Phase 3 — Capture configuration (merged)

Allowed:
- Full-screen and rectangular-region capture.
- Monitor discovery and selection.
- Interactive region selection.

Not allowed:
- Audio capture, GUI, Wayland, or streaming unless separately approved.

### Phase 4 — Audio (current)

Allowed:
- Audio source discovery and selection.
- Microphone and system-audio capture.
- Mixing microphone and system audio into one track.

Not allowed:
- GUI, Wayland, streaming, or separate-track audio unless separately approved.

## Before finishing a task

Run these commands and report the actual result:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo deny check
```

If a command cannot run because a tool is missing or the network is
unavailable, report that explicitly. Never claim a skipped check passed. If a
check fails, investigate the underlying cause. Do not weaken the quality gates
to hide the failure.

## Testing and code quality

- Add unit tests for pure logic.
- Add integration tests when behavior crosses process or filesystem
  boundaries.
- Keep tests deterministic and avoid unnecessary global state.
- Use structured logging where appropriate for application diagnostics.
- Never log captured screen or audio contents.
- Do not add external telemetry or monitoring infrastructure without explicit
  justification and approval.
- Avoid `unwrap()` and `expect()` in production error paths unless there is a
  documented reason.

## Conventions

- Use Conventional Commits.
- The PR title should be suitable as the squash-merge commit message.
- Branch names follow `<type>/<kebab-case-description>`.
- Use Rust Edition 2024 and MSRV 1.91.0.
- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
- Prefer small, reviewable diffs.
- Explain significant design trade-offs rather than silently choosing.
- Avoid unnecessary abstractions.

## Learning requirements

The user is learning Rust through this project. For each implementation
milestone:

1. Explain the design before making changes.
2. Identify the relevant Rust concepts.
3. Explain important ownership, borrowing, and error-handling decisions.
4. Implement only the approved scope.
5. Explain the resulting code.
6. Add and run appropriate tests.
7. Report what the tests prove and what remains unverified.

Introduce concepts progressively rather than generating a large, unexplained
implementation. Ask before generating a large amount of code; a short question
is cheaper than a wrong implementation.

## Conflicts

If a task conflicts with these instructions, explain the conflict and ask for
clarification rather than silently expanding scope.
