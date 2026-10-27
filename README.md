# rust-recorder

A desktop screen recorder for Linux, built in Rust on top of FFmpeg.

> **Status: v0.2.0 release candidate.**
> Command-line recorder for Linux/X11 with video, microphone, system-audio,
> mixed, and audio-only modes, plus graceful Ctrl-C finalization, output
> probing, and structured diagnostics.

## Scope

What works today:

- Full-screen capture, explicit rectangles, monitor selection, and interactive
  region selection on X11.
- H.264/yuv420p video in MP4 or MKV.
- Microphone, system audio, or both mixed into one AAC track; audio-only mode.
- Graceful Ctrl-C finalization, output verification, `ffprobe` checks, and
  structured diagnostics via `RUST_LOG`.

Explicitly out of scope for now: GUI, Wayland, streaming, separate audio
tracks, and multi-monitor composition.

## Requirements

| Dependency | Version | Notes |
| --- | --- | --- |
| Rust | 1.91.0 (pinned) | Installed automatically by `rustup` from `rust-toolchain.toml` |
| FFmpeg | 4.4.2+ | Needs `x11grab`, `libx264`, the `pulse` input, and `ffprobe` |
| xrandr | system utility | Monitor discovery and region bounds validation |
| pactl | `pulseaudio-utils` or `pipewire-pulse` | Audio source discovery and device selection |
| Linux | x86_64, X11 | Only supported target |

## Supported environment

Tested on Pop!_OS 22.04 with GNOME on X11, PipeWire's PulseAudio compatibility
layer, and FFmpeg 4.4.2. Other X11 distributions with FFmpeg and a
PulseAudio-compatible server should work, but are untested. Wayland is not
supported.

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

# Record with the default microphone
rust-recorder --audio mic --output /tmp/mic.mp4

# Record with a specific microphone
rust-recorder --audio mic --microphone-device <ID> --output /tmp/mic.mp4

# Record system audio (an output-monitor source)
rust-recorder --audio system --output /tmp/system.mp4

# Record microphone and system audio mixed into one track
rust-recorder --audio both --output /tmp/both.mp4

# Voice only (no screen capture)
rust-recorder --audio-only --audio mic --output /tmp/voice.mp4

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

## Troubleshooting

- `could not start xrandr` — install `x11-xserver-utils`.
- `could not start pactl` — install `pulseaudio-utils` or `pipewire-pulse`.
- **Bluetooth headset has no microphone** — A2DP provides output only. Switch
  the card to an HFP profile, then restore A2DP afterwards; see
  [docs/audio.md](docs/audio.md#bluetooth-headsets).
- **Recording starts after about a second** — FFmpeg/PulseAudio input startup;
  this is expected.
- **Stopping immediately produces an empty file** — Ctrl-C before the first
  frame cannot finalize a valid recording. Wait at least a second.
- **No audio in the output** — run `--list-audio-devices`, pass the right
  `--microphone-device`/`--system-audio-device`, and confirm with `ffprobe`.

## Known limitations

- Ctrl-C asks FFmpeg to finalize (`q` on stdin) and waits up to 10 seconds
  before killing it. A second Ctrl-C does not shorten that wait.
- Verification checks that the file is non-empty, that a video/audio stream
  exists, and that the duration is positive. Frame-level integrity and playback
  in a specific player are not verified.
- Explicit regions are validated against the desktop rectangle; monitor-derived
  regions are in bounds by construction.
- Audio/video synchronization relies on FFmpeg timestamps; it is not measured.
- Bluetooth profile switching is manual (documented, not automated).
- Audio-only output is MP4/MKV only; `.m4a`/`.mp3` are not accepted yet.
- Linux/X11 only. Wayland is unsupported and untested.
- Release artifacts are Linux x86_64 only and unsigned.
- CI cannot verify real capture or audio; a manual smoke test is required
  (see CONTRIBUTING.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
