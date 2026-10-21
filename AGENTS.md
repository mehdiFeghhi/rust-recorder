# AGENTS.md

Operating instructions for AI coding agents working in this repository. Read
this file before changing anything, then read [CONTRIBUTING.md](CONTRIBUTING.md).

## Project state

Phase 0: engineering foundation only. There is no recording code, no CLI, no
audio, and no GUI. Do not add application features unless the task explicitly
asks for them.

## Hard rules

1. **Stay in scope.** Do not implement capture logic, FFmpeg invocation,
   audio, or GUI code during Phase 0.
2. **No unrequested dependencies.** Every crate needs a justification in the
   PR: problem solved, alternatives, license, MSRV, maintenance status. If the
   license is new, update `deny.toml` deliberately.
3. **Never weaken quality gates.** Do not add blanket `#[allow(...)]`,
   `continue-on-error`, `--no-verify`, or relax `-D warnings` to make a check
   pass. Fix the cause.
4. **Do not commit, push, tag, or publish** unless explicitly asked.
5. **Do not modify files outside this repository.**
6. **No sensitive data.** Never log or embed captured screen or audio content,
   credentials, or tokens. Follow [docs/observability.md](docs/observability.md).
7. **Keep `main` releasable.** Leave the tree formatted, lint-clean, and
   tested.

## Before finishing a task

Run these and report the real result:

```bash
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo deny check
```

If a command cannot run (missing tool, no network), say so explicitly instead
of claiming success. Never describe a check as passing when it was skipped.

## Conventions

- Conventional Commits; the PR title becomes the squash-merge message.
- Branch names: `<type>/<kebab-case-description>`.
- Edition 2024, MSRV 1.91.0. No `unsafe` without prior discussion.
- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
- Prefer small, reviewable diffs, and explain trade-offs rather than silently
  choosing.

## When requirements are ambiguous

Ask before generating a large amount of code. A short question is cheaper than
a wrong implementation.
