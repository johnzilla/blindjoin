---
status: partial
phase: 05-tor-release
source: [05-VERIFICATION.md]
started: 2026-04-09T00:00:00Z
updated: 2026-04-09T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Push v0.1.0 tag and verify GitHub Releases
expected: `release.yml` triggers on `v*` tag push, cross-compiles 4 targets (linux-amd64, linux-arm64, macos-amd64, macos-arm64), uploads tar.gz binaries to GitHub Releases (SC-3)
result: [pending]

### 2. Verify Docker images published to ghcr.io
expected: `docker.yml` triggers on `v*` tag push, builds and pushes coordinator, client, and liquidity-bot images to ghcr.io with multi-arch support (SC-4)
result: [pending]

### 3. Circuit isolation observable verification
expected: Verify that client alice/bob use distinct Tor circuits — ROADMAP SC-2 specifies "verified by integration test against a logging Tor relay." The `isolated_client()` API provides the code-level guarantee; observable verification or override needed.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
