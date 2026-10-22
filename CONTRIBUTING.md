# Contributing to rust-recorder

This project is small on purpose. The rules below exist to keep it
maintainable, not to add ceremony.

## Before you start

- Install Rust with [rustup](https://rustup.rs/). `rust-toolchain.toml` pins
  the exact version; `rustup show` installs it if needed.
- FFmpeg 4.4 or newer is needed only once recording code exists.
- If you are an AI coding agent, read [AGENTS.md](AGENTS.md) first.

## Local setup

```bash
git clone git@github.com:mehdiFeghhi/rust-recorder.git
cd rust-recorder
rustup show
cargo build
cargo test
```

## Branch naming

Format: `<type>/<short-kebab-case-description>`, where `<type>` is one of the
Conventional Commit types (`feat`, `fix`, `docs`, `refactor`, `test`, `ci`,
`chore`, `perf`, `build`).

Examples:

- `feat/x11-capture`
- `fix/ffmpeg-exit-status`
- `docs/observability-policy`

Keep branches short-lived and rebase on `main` before merging. `main` must
always be releasable.

## Commit messages

Follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/):

```text
<type>(<optional scope>): <imperative subject>

<optional body: why, not what>

<optional footer>
```

| Type | Use for |
| --- | --- |
| `feat` | A user-visible capability |
| `fix` | A bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no behavior change |
| `refactor` | Code change that is not a fix or feature |
| `perf` | Performance improvement |
| `test` | Adding or fixing tests |
| `build` | Build system, dependencies, toolchain |
| `ci` | CI configuration |
| `chore` | Maintenance that fits nowhere else |
| `revert` | Reverting a previous commit |

Rules:

- Subject in the imperative mood, lowercase, no trailing period, ≤ 72 chars.
- Body lines wrapped at 72 characters; explain *why*, not *what*.
- Reference issues in the footer: `Refs: #12`.
- Breaking changes: `BREAKING CHANGE: <description>` in the footer.

Examples:

```text
feat(cli): add --duration flag

docs: document the X11 manual smoke test
build(deps): bump clap to 4.5.8
```

## Pull requests

1. Branch from `main` and keep one logical change per PR.
2. Title follows the commit convention; it becomes the squash-merge message.
3. Describe what changed, why, and how you tested it. State limitations.
4. Run the quality gates locally (below). CI must be green.
5. Read your own diff before requesting review.
6. Merge with **squash merge**, then delete the branch.

Squash merging keeps `main` linear and makes `git log` readable. The trade-off
is that individual work commits are not preserved; that is intentional for a
small project.

## Quality gates

```bash
cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo deny check
```

What to do when a gate fails:

- **fmt**: run `cargo fmt --all`, then re-run the check.
- **check**: fix the compile error; never `--offline` your way around it.
- **clippy**: fix the lint. If a lint is genuinely wrong, use a narrowly scoped
  `#[allow(clippy::lint_name)]` with a comment explaining why.
- **test**: fix the code or the test, not the assertion.
- **locked**: a dependency needs updating. Run `cargo update -p <crate>` (or
  `cargo update` for a deliberate refresh), review `Cargo.lock`, and commit it.
- **deny**: see "Dependency management" below.

## Testing requirements

- New logic gets unit tests in the same module (`#[cfg(test)]`).
- Code that spawns or supervises FFmpeg must be testable without a real
  display: put process handling behind a small interface and use a fake
  process in tests.
- Behavior that cannot be tested in CI (actual X11 capture) requires a manual
  smoke test, documented in the PR description: what was recorded, for how
  long, and whether the output played back.
- No coverage threshold yet; tests are judged by whether they would catch a
  realistic regression.

## Changelog policy

This project follows [Keep a Changelog 1.1.0](https://keepachangelog.com/)
and [Semantic Versioning 2.0.0](https://semver.org/).

- Add entries under `## [Unreleased]` in the same PR as the change.
- Use the categories `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`,
  `Security`.
- Write entries for humans: what changed and why it matters, not a commit dump.
- At release time, move the entries into a dated version heading.

## Versioning

`Cargo.toml` `version` is the single source of truth; Git tags are
`vMAJOR.MINOR.PATCH` and must match it.

While the project is pre-1.0 (`0.y.z`):

- Breaking change: bump `y` (e.g. `0.2.0`).
- New feature, backwards compatible: bump `y`.
- Bug fix: bump `z`.

After 1.0.0, standard SemVer applies. Pre-release tags
(`vX.Y.Z-rc.1`) are not handled by the current `Release` workflow; extend its
tag pattern before creating one.

## Release procedure

1. Move `[Unreleased]` entries in `CHANGELOG.md` under a new
   `## [X.Y.Z] - YYYY-MM-DD` heading.
2. Bump `version` in `Cargo.toml`, then run `cargo check` to refresh
   `Cargo.lock` and commit both.
3. Commit as `chore(release): vX.Y.Z`, open a PR, and merge with green CI.
4. Tag the merge commit on `main` and push the tag:

   ```bash
   git switch main && git pull --ff-only
   git tag -a vX.Y.Z -m "rust-recorder vX.Y.Z"
   git push origin vX.Y.Z
   ```

5. The `Release` workflow verifies the tag against `Cargo.toml`, runs tests,
   builds the binary, produces a `.tar.gz` plus `.sha256`, and publishes the
   GitHub Release.
6. Verify the release assets and checksums on the Releases page.

Do not upload binaries by hand; the workflow is the only release path.

## GitHub repository setup (one-time)

Branch protection and required checks are configured in the GitHub UI
(Settings → Rules). Until that is done, nothing prevents direct pushes to
`main`.

- Enable **branch protection** on `main`: require a pull request, require the
  `CI` status checks, require linear history, disallow force pushes.
- Enable **Dependabot alerts** and security updates.
- Enable **private vulnerability reporting** (see SECURITY.md).
- No repository secrets are required. The release workflow uses the automatic
  `GITHUB_TOKEN` with `contents: write`.

## Rust toolchain policy

- The development toolchain is pinned to an exact version in
  `rust-toolchain.toml`; `rust-version` in `Cargo.toml` declares the same
  version as the MSRV. CI uses the pin, so local and CI builds match.
- The pin is deliberate, not neglect: the project currently has no
  dependencies, so the toolchain is only a compatibility choice. Pinning
  avoids silent compiler drift.
- **Upgrade cadence**: review the toolchain at least every six months and
  before any release. Upgrade in a dedicated PR
  (`build(toolchain): bump Rust to X.Y.Z`) that updates both files and runs
  the full gate suite.
- Do not use nightly-only features without opening an issue first.

## Dependency management

The project deliberately has zero dependencies today. Every dependency is a
maintenance and security commitment, so:

- Prefer the standard library and the FFmpeg CLI over a crate.
- A dependency PR must state: the problem it solves, alternatives considered,
  maintenance status, license, MSRV, and rough build-time cost.
- Use normal SemVer requirements, never `*` or open-ended wildcards.
- `Cargo.lock` is committed. CI and releases build with `--locked`.
- Run `cargo deny check` before pushing. It enforces:
  - known advisories (`advisories`),
  - an explicit license allowlist (`licenses`),
  - no duplicate crate versions or wildcard requirements (`bans`),
  - only crates.io and trusted git sources (`sources`).
- `cargo tree -d` lists duplicate versions when `bans` fails.
- Adding a dependency with a new license requires reading that license and
  updating `deny.toml` deliberately, with the reason in the PR. Never add a
  license just to make CI pass.
- Dependabot opens weekly grouped updates. Review the changelog, let CI run,
  and merge; do not blind-merge major bumps.

## Code review

- Today this is a solo project: self-review plus green CI is the minimum.
- When more contributors join: one approving review, and reviewers check
  behavior, error handling, logging against `docs/observability.md`, tests,
  docs, and that no secrets or captured content are logged.
- Reviewers may ask for a smaller PR; that is not a rejection.

## Manual X11 smoke test

CI cannot prove capture works. Before merging a change to capture behavior,
run on a real X11 desktop:

```bash
# Placeholder until the CLI exists:
cargo run -- --output /tmp/smoke.mkv --duration 5
ffprobe /tmp/smoke.mkv
```

Record in the PR: resolution, duration, whether the file plays, and the
FFmpeg exit status. Never attach captured screen content to an issue or PR.

## Known limitations

- X11 only; Wayland is unsupported.
- Single display, video only (no audio) in the first iteration.
- Release artifacts are Linux x86_64 only and unsigned.
