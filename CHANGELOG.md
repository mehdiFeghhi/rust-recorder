# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Command-line recording: `rust-recorder --output <PATH> [--fps N]
  [--duration SECONDS]` builds an FFmpeg `x11grab`/`libx264` command and reports
  its exit status.
- Ctrl-C stops recording gracefully: FFmpeg is asked to finalize the container
  and gets up to 10 seconds before being killed.
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
