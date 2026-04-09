---
phase: 05-tor-release
verified: 2026-04-09T21:00:00Z
status: human_needed
score: 6/8 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Push a v0.1.0 tag to origin and verify GitHub Releases shows four tar.gz artifacts"
    expected: "blindjoin-linux-amd64.tar.gz, blindjoin-linux-arm64.tar.gz, blindjoin-macos-amd64.tar.gz, blindjoin-macos-arm64.tar.gz appear in the GitHub Releases page for the tag"
    why_human: "No v* tag has been pushed yet. The release.yml workflow exists and is correctly configured, but no actual GitHub Release or binary artifacts have been published. SC-3 requires artifacts to be 'downloadable' — they are not yet."
  - test: "Verify ghcr.io has a published coordinator image after the docker.yml workflow runs"
    expected: "docker pull ghcr.io/johnzilla/blindjoin-coordinator:latest succeeds and the image passes a signet smoke test (coordinator starts and logs .onion or TCP listener address)"
    why_human: "No v* tag or main-branch push has triggered the Docker workflow since the workflow files were added. No image exists at ghcr.io yet. SC-4 requires the image to actually be 'hosted' at ghcr.io."
  - test: "Verify circuit isolation using a logging Tor relay (SC-2 from ROADMAP)"
    expected: "Input registration and output registration appear on different Tor circuits — observable via a test relay that logs circuit IDs"
    why_human: "The code correctly uses TorClient::isolated_client() which guarantees circuit isolation at the arti API level. However, the ROADMAP SC-2 explicitly says 'verified by integration test against a logging Tor relay'. No such integration test exists in the codebase — the isolation is enforced by arti's API contract, not verified by an observable test."
---

# Phase 5: Tor & Release Verification Report

**Phase Goal:** The coordinator runs as a Tor v3 hidden service with no clearnet endpoint; participants use fresh Tor circuits per phase; pre-built binaries and container images are publicly available
**Verified:** 2026-04-09T21:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | When tor_mode = true, coordinator binds exclusively to a Tor v3 .onion address with no TCP listener | VERIFIED | `coordinator/src/main.rs` lines 150-191: if/else block — tor path calls `serve_onion_service()` and never calls `TcpListener::bind`; TcpListener only appears at line 182 inside the `else` branch |
| 2 | The .onion address is passed to PKARR publisher via oneshot channel | VERIFIED | `coordinator/src/main.rs` lines 151, 171, 192-216: `oneshot::channel()` created in tor branch; `addr_rx.await` resolves to `public_addr`; PKARR initial publish and heartbeat both use `public_addr.clone()` |
| 3 | When tor_mode = false, coordinator behaves identically to Phase 4 (TCP listener only) | VERIFIED | Else branch (lines 176-190) binds TcpListener and uses `cfg.discovery.coordinator_public_addr` — identical to Phase 4 behavior; 47 unit tests confirmed passing in SUMMARY |
| 4 | cargo build --release -p coordinator succeeds with arti-client deps | VERIFIED | Commits 5327abf and ce42674 confirmed in git log; SUMMARY-01 documents successful build with SQLITE3_LIB_DIR workaround |
| 5 | Client uses a distinct Tor circuit for input registration (alice) and output registration (bob) | VERIFIED (code) | `client/src/tor.rs`: `TorHandle` has `alice: TorClient<PreferredRuntime>` and `bob: TorClient<PreferredRuntime>` obtained via `base.isolated_client()`. `client/src/http.rs` line 75: `post_output` uses `self.bob()` which returns `bob_client`. Circuit isolation is enforced by arti API contract. Integration test against a logging Tor relay does not exist — see Human Verification item 3. |
| 6 | --tor flag absent or test mode: CoordinatorClient falls back to plain reqwest (no Tor) | VERIFIED | `client/src/http.rs`: `new()` creates `alice_client: Client::new()` with `bob_client: None`; all methods fall through without proxy. `client/src/main.rs` line 67: `if cfg.use_tor` branch gates Tor initialization. |
| 7 | GitHub Releases contains downloadable Linux and macOS binaries (SC-3) | NOT VERIFIED | `.github/workflows/release.yml` exists with correct matrix (4 targets, cross-rs for aarch64, softprops/action-gh-release@v1). YAML is syntactically valid. BUT: no v* tag has been pushed to origin (`git tag` shows empty). No GitHub Release or binary artifacts actually exist. Workflow infrastructure is correct; execution has not occurred. |
| 8 | ghcr.io hosts a coordinator Docker image passing a signet smoke test (SC-4) | NOT VERIFIED | `.github/workflows/docker.yml` exists with correct multi-arch matrix (3 images, linux/amd64+linux/arm64, ghcr.io push). YAML valid. `docker/Dockerfile.client` exists with cargo-chef multi-stage pattern. BUT: no trigger event has occurred (no v* tag, and docker.yml runs on main-branch push too — unclear if the workflow commit itself triggered a push; no evidence of published image). |

**Score:** 6/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `coordinator/src/network/tor.rs` | `serve_onion_service(app, addr_tx)` function | VERIFIED | Exists, 99 lines, substantive implementation with full accept loop. Imports: arti_client, tor_hsservice, tor_cell, safelog, hyper_util, hyper, futures_util. |
| `coordinator/src/config.rs` | `tor_mode: bool` field in CoordinatorSection | VERIFIED | Line 31: `pub tor_mode: bool` with `#[serde(default)]` at line 29 context. Default `false` at line 120. |
| `coordinator/src/main.rs` | Branching logic on tor_mode | VERIFIED | Lines 150-191: if/else on `cfg.coordinator.tor_mode`; imports `serve_onion_service` at line 21. |
| `coordinator/src/network/mod.rs` | Declares `pub mod tor` | VERIFIED | Single line: `pub mod tor;` |
| `coordinator/src/lib.rs` | Declares `pub mod network` | VERIFIED | Line 7: `pub mod network;` |
| `client/src/tor.rs` | `TorHandle` with alice/bob isolation and `init_tor()` | VERIFIED | 214 lines. `TorHandle` struct at line 30 with `alice` and `bob` fields. `init_tor()` at line 86. In-process SOCKS5 proxy at lines 96-125. Full bidirectional relay at lines 204-211. |
| `client/src/http.rs` | `new_tor()` constructor; `post_output` uses bob circuit | VERIFIED | `new_tor()` at line 32. `bob()` helper at line 47. `post_output` uses `self.bob()` at line 75. All other methods use `alice_client`. |
| `client/src/config.rs` | `use_tor: bool` field | VERIFIED | Line 56: `pub use_tor: bool` |
| `client/src/main.rs` | `--tor` flag wires to `init_tor()` + `new_tor()` | VERIFIED | `mod tor` at line 8. `if cfg.use_tor` branch at line 67. |
| `client/src/lib.rs` | `pub mod tor` declared | VERIFIED | Line 9: `pub mod tor;` |
| `.github/workflows/release.yml` | Matrix CI, v* tag trigger, cross-rs, softprops upload | VERIFIED | File exists, 76 lines. 4-target matrix. `softprops/action-gh-release@v1`. `aarch64-unknown-linux-gnu` with `use_cross: true`. YAML valid. |
| `.github/workflows/docker.yml` | Multi-arch, ghcr.io push, 3 images | VERIFIED | File exists, 60 lines. 3-image matrix. `docker/build-push-action@v7` with `platforms: linux/amd64,linux/arm64`. YAML valid. |
| `docker/Dockerfile.client` | cargo-chef multi-stage build for client binary | VERIFIED | File exists, 28 lines. cargo-chef pattern: planner/builder/runtime stages. `cargo build --release --bin client`. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `coordinator/src/network/tor.rs` | `coordinator/src/main.rs` | oneshot channel carrying String (.onion address) | WIRED | `main.rs` line 151: `oneshot::channel::<String>()`. Line 171: `serve_onion_service(app, addr_tx)` spawned. Line 174: `addr_rx.await`. `tor.rs` line 63: `addr_tx.send(onion_addr)`. |
| `.onion address (from tor.rs)` | `discovery::pkarr_pub::build_coordinator_packet` | `public_addr` variable in main.rs before PKARR publish | WIRED | `main.rs` lines 192-215: initial publish uses `addr = public_addr.clone()`. Line 216: heartbeat uses `let addr = public_addr.clone()`. |
| `client/src/tor.rs (TorHandle)` | `client/src/http.rs (CoordinatorClient)` | `alice_proxy_url()` / `bob_proxy_url()` Strings passed to `new_tor` | WIRED | `main.rs` lines 68-72: `handle.alice_proxy_url().await?` and `handle.bob_proxy_url().await?` passed to `new_tor()`. `http.rs` line 32-44: `Proxy::all()` on both. |
| `client/src/main.rs --tor flag` | `client/src/tor.rs init_tor()` | `cfg.use_tor` branch in main | WIRED | `main.rs` line 67: `if cfg.use_tor { let handle = tor::init_tor(coordinator_url.clone()).await`. |
| `.github/workflows/release.yml` | GitHub Releases | `softprops/action-gh-release@v1` | CONFIGURED (not yet executed) | Workflow file is correct but no v* tag has been pushed. Link exists in code but has not been traversed. |
| `.github/workflows/docker.yml` | `ghcr.io/$owner/blindjoin-coordinator` | `docker/build-push-action@v7` | CONFIGURED (not yet executed) | Same as above — no trigger event has caused an image push. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `coordinator/src/network/tor.rs` | `onion_addr: String` | `onion_service.onion_address()` — arti TorClient returns actual .onion address from Tor network | Yes (when Tor network reachable) | FLOWING |
| `client/src/tor.rs` | `alice`, `bob` TorClient handles | `base.isolated_client()` — creates handles with fresh IsolationToken | Yes (real circuit isolation) | FLOWING |
| `client/src/http.rs` | HTTP responses | `self.alice_client` / `self.bob()` via SOCKS5 proxy to real coordinator | Yes (real network requests) | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| coordinator/src/network/tor.rs contains serve_onion_service | `grep -c "pub async fn serve_onion_service" coordinator/src/network/tor.rs` | 1 | PASS |
| TcpListener only in clearnet branch of main.rs | `grep -n "TcpListener" coordinator/src/main.rs` shows only line 182 (inside else block) | 1 occurrence inside else | PASS |
| post_output uses bob circuit | `grep -n "self.bob()" client/src/http.rs` at post_output line 75 | Confirmed | PASS |
| YAML workflows valid | `python3 yaml.safe_load` on both files | Both valid | PASS |
| All phase commits exist | `git log --oneline` shows 5327abf, ce42674, 52fb6e9, fd0cee4, 1a721f4, 71b5caa | All 6 commits present | PASS |
| No v* tag pushed (releases not yet triggered) | `git tag` output | Empty | FAIL — SC-3 and SC-4 not yet achieved in production |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| PRIV-03 | 05-01-PLAN.md | Coordinator runs as Tor hidden service via arti-client (no clearnet endpoint in production) | SATISFIED | `coordinator/src/network/tor.rs` implements onion service. `coordinator/src/config.rs` has `tor_mode: bool`. `coordinator/src/main.rs` gates TCP vs Tor — TcpListener only in `else` branch. |
| CLI-05 | 05-02-PLAN.md | Fresh Tor circuit per phase (input registration circuit != output registration circuit) | SATISFIED (code) / NEEDS HUMAN (observable test) | `client/src/tor.rs`: `isolated_client()` for alice and bob. `client/src/http.rs`: `post_output` uses bob circuit. The ROADMAP SC-2 qualifier "verified by integration test against a logging Tor relay" is not met — no such test exists. |
| DEPL-03 | 05-03-PLAN.md | Pre-built Linux/macOS binaries via GitHub Releases (GitHub Actions CI) | INFRASTRUCTURE COMPLETE / EXECUTION PENDING | `release.yml` workflow exists with correct 4-target matrix and `softprops/action-gh-release@v1`. No v* tag has been pushed — no actual release artifacts exist at GitHub Releases. |
| DEPL-04 | 05-03-PLAN.md | Docker images published to GitHub Container Registry (ghcr.io) | INFRASTRUCTURE COMPLETE / EXECUTION PENDING | `docker.yml` workflow exists with correct multi-arch matrix and ghcr.io push. `Dockerfile.client` exists. No image has been published. |

### Anti-Patterns Found

No anti-patterns found in the key modified files. No TODO/FIXME/placeholder comments. No empty implementations or stub returns. No hardcoded empty data structures in rendering paths.

### Human Verification Required

#### 1. Trigger Release Workflow by Pushing a Version Tag

**Test:** `git tag v0.1.0 && git push origin v0.1.0` — then check the Actions tab on GitHub for the Release workflow run, and check the Releases page for blindjoin-linux-amd64.tar.gz, blindjoin-linux-arm64.tar.gz, blindjoin-macos-amd64.tar.gz, blindjoin-macos-arm64.tar.gz.

**Expected:** All four matrix jobs complete successfully. Four tar.gz files appear under the v0.1.0 release. Each contains a `coordinator-{name}` and `client-{name}` binary. The linux-arm64 job uses cross-rs.

**Why human:** No v* tag has been pushed. The workflow configuration is correct and has been validated by YAML parsing, but the actual CI execution has not occurred. GitHub Actions (Linux aarch64 with cross-rs + arti-client + sqlite) may surface linker or build issues not visible from local inspection. The SQLITE3_LIB_DIR workaround documented in SUMMARY-01 and SUMMARY-02 may need to be handled in the Docker build context for cross-rs — the cross-rs Docker image may lack libsqlite3-dev.

#### 2. Verify Docker Images Published to ghcr.io

**Test:** Confirm the Docker workflow ran (check GitHub Actions on the commits that added docker.yml, or push a tag or merge to main if it hasn't run). Then: `docker pull ghcr.io/johnzilla/blindjoin-coordinator:latest` and run with `--network signet` against a signet bitcoind, verify startup log shows Tor or TCP listener.

**Expected:** `docker pull` succeeds. Container starts and either logs `.onion` address (if `BLINDJOIN_TOR_MODE=true`) or `Listening (clearnet)`. cargo-chef dependency caching means rebuild is fast.

**Why human:** No evidence that docker.yml has been triggered — the workflow was added in commit 71b5caa but the `on: push: branches: [main]` trigger only fires on main-branch pushes. Need to verify if the commit push to main triggered the workflow, and if the image is available at ghcr.io. Also, the Dockerfile.client must be verified to build correctly for linux/arm64 under QEMU (SQLITE3_LIB_DIR issue may affect ARM64 image build).

#### 3. Circuit Isolation Observable Test (ROADMAP SC-2)

**Test:** Run the client with `--tor` against a coordinator, and observe via a Tor relay log or traffic analysis tool that the circuits used for input registration and output registration are distinct.

**Expected:** The two HTTP requests (POST /round/input and POST /round/output) traverse different Tor circuits — different guard nodes, different circuit IDs.

**Why human:** The ROADMAP explicitly states this must be "verified by integration test against a logging Tor relay." No such test exists in the test suite. The code guarantee is strong (`isolated_client()` is the correct arti API for circuit isolation), but the ROADMAP's success criterion specifically requires observable verification, not just API-level correctness.

### Gaps Summary

There are no code-level gaps — all required files exist with substantive implementations, all key links are wired, and no anti-patterns were found.

The 2 unverified must-haves (SC-3: GitHub Releases, SC-4: ghcr.io images) represent a gap between "workflow infrastructure is ready" and "artifacts are publicly available." The phase goal says "pre-built binaries and container images are **publicly available**" — this is not yet true because no v* tag has been pushed and no images have been published.

The ROADMAP SC-2 qualifier (integration test against a logging Tor relay) has not been implemented — it exists as a code-level guarantee via `isolated_client()` but the observable verification specified in the success criterion is missing.

**Recommended immediate action:** Push a `v0.1.0` tag to trigger both workflows. Monitor the Actions tab for the cross-rs aarch64 build (the SQLITE3_LIB_DIR issue may need a `Cross.toml` or apt step). If both workflow runs succeed, SC-3 and SC-4 will be satisfied and this phase can be re-verified as `passed` (with SC-2 carrying an override for the logging relay test, since the arti API provides the isolation guarantee).

---

_Verified: 2026-04-09T21:00:00Z_
_Verifier: Claude (gsd-verifier)_
