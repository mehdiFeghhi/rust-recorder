# rust-recorder

A desktop screen recorder for Linux, built in Rust on top of FFmpeg.

> **Status: Phase 0 — engineering foundation.**
> The application does not record anything yet. This phase establishes the
> toolchain, CI, dependency policy, release process, and documentation that
> application work will build on.

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
| FFmpeg | 4.4.2 (system) | Runtime dependency; not needed until recording lands |
| Linux | x86_64, X11 | Only supported target in Phase 0 |

## Getting started

```bash
git clone <repository-url>
cd rust-recorder
rustup show            # installs the pinned toolchain and components
cargo build
cargo run
```

Expected output for now: `Hello, world!`

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
- [AGENTS.md](AGENTS.md) — instructions for AI coding agents

## Known limitations

- Nothing is implemented yet: no recording, no CLI, no audio.
- Linux/X11 only. Wayland is unsupported and untested.
- CI verifies formatting, compilation, lints, tests, and packaging. It cannot
  verify that screen capture works on a real desktop; that needs a manual
  smoke test (see CONTRIBUTING.md).
- No released versions and no GitHub remote yet.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
