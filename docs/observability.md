# Observability policy

This document defines how `rust-recorder` will log and diagnose itself. It is a
policy, not an implementation: **no logging dependency is added in Phase 0**.
The plan below is executed when the first application code lands.

## Goals

- Explain what the recorder did, and why it failed, from logs alone.
- Make production failures reproducible without access to the user's screen.
- Keep the diagnostic surface small enough to read.

## Non-goals

- No telemetry, monitoring server, or third-party analytics. The binary makes
  no network connections.
- No logging of captured screen contents, audio samples, or full file
  contents.
- No metrics endpoint or tracing export in the first iteration. Structured
  logs on stderr are enough for a single-process CLI.

## Stack

Logging uses:

| Crate | Role |
| --- | --- |
| `tracing` | Structured events and spans, with fields instead of string formatting |
| `tracing-subscriber` | Formatting and filtering, configured from `RUST_LOG` |

Why `tracing` over `log`: spans let one recording session carry context
(session ID, output path, duration) across calls without threading a logger
through every function. The `log` facade is simpler, but this project is
already async/process-oriented and will benefit from structured fields.

Trade-off: two crates and a small runtime cost, plus a learning curve. That is
acceptable now that there is real lifecycle code to observe.

## Log levels

| Level | Meaning | Examples |
| --- | --- | --- |
| `ERROR` | The requested operation failed and the user must act. Always paired with a non-zero exit. | FFmpeg could not be spawned; output file could not be finalized; capture failed mid-recording. |
| `WARN` | Something is degraded but recording can continue, or a recovery happened. | Frame rate requested but the display reports a different one; FFmpeg wrote a warning; falling back to a default codec; stale output file overwritten. |
| `INFO` | The intended lifecycle of the application. Low volume, one or two lines per session. | Recording started (`output`, `display`, `fps`); recording stopped (`duration_ms`, `exit_code`); finalization succeeded. |
| `DEBUG` | Detail useful when reproducing a report. Off by default. | Full FFmpeg command line, argument values, resolved configuration, process IDs, retry attempts. |
| `TRACE` | Very granular internal state, expected to be noisy. | Per-event state transitions, internal buffer/queue decisions. Never per-frame. |

Rules of thumb:

- If a user would need to change something, it is `WARN` or `ERROR`.
- If it happens once per recording, `INFO` is fine.
- If it happens per frame or per sample, it is not a log — use `TRACE` only if
  it is cheap, and prefer counters or summaries.
- Never use `ERROR` for conditions that are handled and do not affect the user.

## Event catalog

These are the events the application is expected to emit. Field names are
stable once implemented; add fields rather than renaming them.

| Event | Level | Fields |
| --- | --- | --- |
| `recording.start` | INFO | `output`, `display`, `fps`, `duration_limit` |
| `recording.stop` | INFO | `duration_ms`, `reason` (user, duration, signal) |
| `recording.finalize` | INFO | `success`, `duration_ms`, `output_size_bytes` |
| `ffmpeg.spawn_failed` | ERROR | `error`, `program` |
| `ffmpeg.exit` | INFO / ERROR | `exit_code`, `signal`, `stderr_tail` |
| `capture.interrupted` | WARN | `signal` |
| `config.invalid` | ERROR | `field`, `reason` |

Spans: one span per recording session, carrying `session.id`, so every event
outside the session loop is attributable.

## Sensitive data rules

- Never log raw pixels, encoded frames, audio samples, or file contents.
- Paths are acceptable but may contain a username. Log the path the user
  passed; never crawl directories to discover more.
- FFmpeg `stderr` is logged only as a bounded tail (for example, the last 20
  lines) and only at `DEBUG`/on failure. FFmpeg may echo input metadata;
  treat it as potentially sensitive.
- Environment dumps and `env` variables are not logged.
- Crash reports, if added later, must be scrubbed with the same rules.

## Output and filtering

- All diagnostics go to **stderr**. `stdout` is reserved for machine-readable
  output once the CLI produces any.
- Default filter is `warn`, so normal CLI output stays clean.
  `RUST_LOG=rust_recorder=info` enables lifecycle events and `debug` adds
  FFmpeg process detail. A future `--verbose` flag may map onto this.
- Format: human-readable single lines by default. A `--log-format json` option
  may be added if log collection becomes necessary; not planned now.
- No log files are written by default. If file logging is added, it must be
  opt-in, size-capped, and documented.

## What a bug report should contain

When asking for help, request:

1. `rust-recorder --version` output.
2. `ffmpeg -version` first line.
3. The failing command line (redact personal paths if needed).
4. The `ERROR`/`WARN` lines and the FFmpeg exit code.
5. Whether the output file exists and its size.

Do not ask for or attach screen recordings.

## Adoption status

- [x] `tracing` and `tracing-subscriber` with a `RUST_LOG`-based filter.
- [x] Subscriber initialized once in `main`, writing to stderr.
- [x] Session span (`session_id`, `output`, `fps`, `display`) and lifecycle
      events (start, finish, interrupt, exit, shutdown timeout).
- [x] No captured screen or audio content is logged.
- [ ] `--verbose` / `-vv` flags (use `RUST_LOG` for now).
