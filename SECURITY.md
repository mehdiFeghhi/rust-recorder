# Security Policy

## Supported versions

No versions have been released yet. Once releases exist, only the latest
`0.x` release receives security fixes until a `1.0` line is established.

| Version | Supported |
| --- | --- |
| unreleased `main` | Best effort |
| released `0.x` | Latest release only |

## Reporting a vulnerability

**Do not open a public issue.**

- Preferred: use GitHub's [private vulnerability reporting][gh-pvr]
  (Security tab → "Report a vulnerability"). This keeps the report private
  until a fix is available.
- If the repository is not yet on GitHub, contact the maintainer directly.

Include:

- Affected version or commit.
- Operating system, desktop session (X11/Wayland), and FFmpeg version.
- Reproduction steps and the observed impact.
- Relevant logs **with screen contents, file paths, and other personal data
  removed**.

[gh-pvr]: https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability

## What to expect

This is a personal project maintained on a best-effort basis. The goal is to
acknowledge a report within 7 days and agree on a disclosure timeline. That is
an intention, not a contractual SLA.

## Scope

In scope:

- The `rust-recorder` binary and its command-line interface.
- How the binary invokes, supervises, and cleans up FFmpeg processes.
- The build and release pipeline (`.github/workflows/`), including artifact
  integrity and unintended secret exposure.

Out of scope:

- Vulnerabilities in FFmpeg itself; report those to the FFmpeg project.
- Issues that require an already-compromised desktop session or root access.
- Denial of service caused by deliberately oversized output files on a local
  machine.

## Handling secrets and sensitive data

- The repository contains no secrets. The release workflow uses the automatic
  `GITHUB_TOKEN`; no custom secrets are configured.
- Never commit tokens, credentials, or recorded screen/audio content.
- Logs must follow [docs/observability.md](docs/observability.md): no captured
  pixels, audio samples, or sensitive user data.

## Dependency vulnerabilities

- `cargo deny check advisories` runs in CI and fails on known advisories.
- Dependabot alerts and security updates are enabled once the repository is on
  GitHub.
- If an advisory affects a dependency, the fix is a dependency update plus a
  `Security` entry in `CHANGELOG.md`.
