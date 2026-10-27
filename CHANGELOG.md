# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Mixed audio: `--audio both` records microphone and system audio into one
  AAC track with FFmpeg `amix`.
- System-audio recording: `--audio system` captures an output-monitor source,
  selectable with `--system-audio-device <ID>`.
- Audio-aware verification: when audio is enabled, `ffprobe` must find an audio
  stream before success is reported.
- Microphone recording: `--audio mic` records the default microphone, and
  `--microphone-device <ID>` selects one from `--list-audio-devices`.
- Audio source discovery: `--list-audio-devices` reports PulseAudio-compatible
  sources and classifies microphones versus output monitors.
- Interactive selection: `--select-region` uses FFmpeg `x11grab`'s pointer
  selection, so no GUI code is needed.
- Region bounds validation: an explicit `--region` must fit within the
  connected desktop; out-of-bounds rectangles are rejected before FFmpeg runs.
- Monitor selection: `--monitor N` records one monitor from
  `--list-monitors`, translating its geometry into a capture region.
- Monitor discovery: `--list-monitors` reports connected outputs, their
  geometry, and the primary flag.
- Region capture: `--region WIDTHxHEIGHT+X+Y` records a rectangle of the
  display instead of the full screen.
- `--version` / `-V` flag.
- Command-line recording: `rust-recorder --output <PATH> [--fps N]
  [--duration SECONDS]` builds an FFmpeg `x11grab`/`libx264` command and reports
  its exit status.
- Ctrl-C stops recording gracefully: FFmpeg is asked to finalize the container
  and gets up to 10 seconds before being killed.
- Post-recording sanity check: the output file must exist, be a regular file,
  and contain data before success is reported.
- Structured diagnostics with `tracing`: a session span plus start, finish,
  interrupt, exit, and shutdown-timeout events, filtered by `RUST_LOG` and
  written to stderr.
- `ffprobe` verification: the recording must contain a readable video stream
  and a positive duration before success is reported.
- Engineering foundation: pinned Rust toolchain, CI quality gates
  (format, check, Clippy, tests, release build), dependency policy enforced by
  `cargo-deny`, weekly Dependabot updates, and a tag-driven release workflow
  producing a `.tar.gz` and SHA-256 checksum.
- Contributor, security, and observability documentation, plus agent
  instructions in `AGENTS.md`.
- Dual MIT / Apache-2.0 licensing.

<!--
Release entries look like:

## [0.1.0] - YYYY-MM-DD

### Added
- ...
-->
