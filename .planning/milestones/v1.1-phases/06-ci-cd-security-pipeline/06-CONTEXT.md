# Phase 6: CI/CD Security Pipeline - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Add automated test, lint, and security scanning gates to the GitHub Actions CI/CD pipeline. Every PR must pass cargo test, cargo clippy, and cargo audit before merge. Release workflows must also gate on test/clippy success.

</domain>

<decisions>
## Implementation Decisions

### Workflow Structure
- **D-01:** Create a new `ci.yml` workflow triggered on pull requests, running cargo test, cargo audit, and cargo clippy
- **D-02:** Add test and clippy prerequisite jobs to `release.yml` and `docker.yml` so tagged releases cannot ship broken code
- **D-03:** The existing release.yml and docker.yml remain tag-triggered; ci.yml is PR-triggered

### Audit Policy
- **D-04:** `cargo audit` fails CI only on critical and high severity advisories — low and medium are warnings, not blockers
- **D-05:** Use `cargo audit` severity filtering (e.g., `--deny warnings` is NOT used; instead configure to deny only high+)

### Branch Protection
- **D-06:** Do NOT programmatically set branch protection rules via gh CLI
- **D-07:** Include setup instructions (README section or workflow comment) documenting how to enable required status checks in GitHub repo settings

### Claude's Discretion
- Specific GitHub Actions versions and caching strategy
- Whether to use a reusable workflow or inline steps
- Rust toolchain pinning approach (stable vs specific version)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### CI/CD Workflows
- `.github/workflows/release.yml` — Current release workflow (tag-triggered, no tests)
- `.github/workflows/docker.yml` — Current Docker build workflow (tag-triggered, no tests)

### Project Structure
- `Cargo.toml` — Workspace root defining 4 crates (shared, coordinator, client, liquidity-bot)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `dtolnay/rust-toolchain@stable` — already used in release.yml, reuse in ci.yml
- `Swatinem/rust-cache@v2` — already used in release.yml for build caching
- `docker/setup-buildx-action@v4` — buildx already set up in docker.yml

### Established Patterns
- Tag-triggered workflows (`on: push: tags: ['v*']`) for release and Docker
- Matrix strategy used in docker.yml for multi-image builds
- `softprops/action-gh-release@v1` for GitHub Releases

### Integration Points
- ci.yml needs to trigger on `pull_request` against `main`
- release.yml needs a `check` job that runs test+clippy before the `build` job
- docker.yml needs a similar prerequisite check job before the `docker` matrix

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard CI pipeline patterns apply.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-ci-cd-security-pipeline*
*Context gathered: 2026-04-09*
