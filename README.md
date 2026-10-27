# rust-recorder

A desktop screen recorder for Linux, built in Rust on top of FFmpeg.

> **Status: Phase 1 — command-line prototype (in progress).**
> The CLI validates arguments, builds an FFmpeg `x11grab` command, runs it to
> completion, finalizes the file gracefully on Ctrl-C, verifies the file, and
> probes it with `ffprobe` for a video stream and a positive duration. Audio is
> not implemented yet.

## Scope

Planned for the first usable release:

- Capture one X11 display to a video file using FFmpeg.
- A small command-line interface for output path, frame rate, and duration.

Explicitly out of scope for now: audio capture, GUI, streaming, Wayland, and
multi-monitor composition.

## Requirements

| Dependency | Version | Notes |
| --- | --- | --- |
| Rust | 1.91.0 (pinned) | Installed automatically by `rustup` from `rust-toolchain.toml` |
| FFmpeg | 4.4.2 (system) | Required at runtime for recording and probing (`ffprobe`) |
| Linux | x86_64, X11 | Only supported target |
| xrandr | system utility | Monitor discovery and region bounds validation |

## Getting started

```bash
git clone git@github.com:mehdiFeghhi/rust-recorder.git
cd rust-recorder
rustup show            # installs the pinned toolchain and components
cargo build
cargo run -- --output /tmp/demo.mp4 --duration 5
```

FFmpeg records the display for the requested duration and writes the file; the
process exit status is reported. Diagnostics are off by default; use
`RUST_LOG=rust_recorder=info` (or `debug`) to see the recording lifecycle and
FFmpeg process detail. A missing `ffmpeg` binary, an unreachable
`DISPLAY`, or an FFmpeg failure produces a nonzero exit status.

## Usage

```bash
# Record the whole display for 10 seconds
rust-recorder --output /tmp/demo.mp4 --duration 10

# List connected monitors
rust-recorder --list-monitors

# List audio capture sources
rust-recorder --list-audio-devices

# Record the first monitor
rust-recorder --monitor 0 --output /tmp/monitor.mp4

# Pick a region with the mouse, then record
rust-recorder --select-region --output /tmp/selection.mp4

# Record a 640x480 rectangle at 100,80
rust-recorder --region 640x480+100+80 --output /tmp/region.mp4

# Record until Ctrl-C
rust-recorder --output /tmp/demo.mp4

# Print the version
rust-recorder --version
```

Requires an X11 session and `ffmpeg` / `ffprobe` on `PATH`.

## Quality gates

Run the same checks CI runs before opening a pull request:

```bash
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo deny check        # dependency policy; see CONTRIBUTING.md
```

`cargo deny` is a separate binary:

```bash
cargo install cargo-deny --locked   # or: cargo binstall cargo-deny
```

## Repository layout

| Path | Contents |
| --- | --- |
| `src/` | Application source (placeholder `main.rs` in Phase 0) |
| `.github/workflows/` | CI and release automation |
| `docs/observability.md` | Logging and diagnostics policy |
| `deny.toml` | Dependency policy enforced by cargo-deny |
| `rust-toolchain.toml` | Pinned development toolchain |
| `AGENTS.md` | Rules for AI coding agents |

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md) — workflow, branch/commit conventions, releases
- [SECURITY.md](SECURITY.md) — vulnerability reporting
- [CHANGELOG.md](CHANGELOG.md) — notable changes
- [docs/observability.md](docs/observability.md) — logging and diagnostics policy
- [docs/audio.md](docs/audio.md) — audio subsystem design
- [AGENTS.md](AGENTS.md) — instructions for AI coding agents

## Known limitations

- Ctrl-C asks FFmpeg to finalize (`q` on stdin) and waits up to 10 seconds
  before killing it. A second Ctrl-C does not shorten that wait.
- Verification checks that the file is non-empty and that `ffprobe` can read
  a video stream with a positive duration. Frame-level integrity and playback
  in a specific player are not verified.
- Explicit regions are validated against the desktop rectangle before capture;
  monitor-derived regions are in bounds by construction.
- No audio capture yet.
- Linux/X11 only. Wayland is unsupported and untested.
- CI verifies formatting, compilation, lints, tests, and packaging. It cannot
  verify that screen capture works on a real desktop; that needs a manual
  smoke test (see CONTRIBUTING.md).
- No released versions yet (v0.1.0 is planned after Phase 1).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
