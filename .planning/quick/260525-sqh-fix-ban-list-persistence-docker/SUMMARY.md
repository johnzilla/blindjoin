---
quick_id: 260525-sqh
slug: fix-ban-list-persistence-docker
workstream: fix-ban-list-persistence
created: 2026-05-26
completed: 2026-05-26
status: complete
pr_url: https://github.com/johnzilla/blindjoin/pull/3
branch: fix-ban-list-persistence-docker
base_commit: ca61348
ci_run: https://github.com/johnzilla/blindjoin/actions/runs/26425925023
---

# Summary

Ban list now persists across coordinator restarts in the Docker deployment. The `BLAME-05` contract (documented in `coordinator/src/config.rs:24`) is now honored end-to-end: Rust code already loaded from `ban_file_path` on startup, Docker now mounts a persistent volume to back it.

## Commits (on branch `fix-ban-list-persistence-docker`)
- `bfc171d` — `docker: add coordinator-data volume for ban list persistence`
  - `docker/docker-compose.yml`: new `coordinator-data:/app/data` mount + `BLINDJOIN__COORDINATOR__BAN_FILE_PATH=/app/data/ban_list.jsonl` env var + named volume declaration.
  - `docker/Dockerfile`: `RUN mkdir -p /app/data` in the coordinator target (mirrors existing `/app/keys` pattern).
- `3c4c9a6` — `test: ban list persists across coordinator restart`
  - New `tests/integration/ban_list_persistence.rs` with 4 cases: happy-path persistence, expired-entry filtering, missing-file first-startup, persisted-hash / in-memory-key-format equivalence.
  - Registered in `tests/integration/mod.rs`.

## PR
[apps#3 — fix: persist ban list across coordinator restarts in Docker](https://github.com/johnzilla/blindjoin/pull/3)

## CI verdict
All 4 jobs green on [run #26425925023](https://github.com/johnzilla/blindjoin/actions/runs/26425925023):

| Job | Result |
|---|---|
| cargo test (incl. new `ban_list_persistence` cases) | ✓ pass |
| cargo clippy | ✓ pass |
| coordinator binary builds | ✓ pass |
| cargo audit | ✓ pass |

## Test approach
Public API — no file-format-direct write needed. `append_ban_entry`, `load_unexpired_entries`, `BanList`, `BanEntry`, `hash_utxo_str`, `now_unix_secs` are all already `pub` in `coordinator::round::blame`. Test uses the exact write path of `on_signing_timeout` ([coordinator/src/round/blame.rs:213](coordinator/src/round/blame.rs:213)) and the exact read path of `coordinator::run` ([coordinator/src/run.rs:70](coordinator/src/run.rs:70)) — so the test exercises real production code paths, not synthetic ones. No bitcoind, no HTTP, no Tor.

## Deviations
None of substance. Skipped optional commit 3 (Dockerfile `mkdir` was a one-line change, folded into commit 1). Did not attempt manual `docker compose` cycle — structurally identical to the working `coordinator-keys` mount, and full Docker round-trip belongs in a follow-on if anyone wants belt-and-suspenders validation.

## Next
Merge [PR #3](https://github.com/johnzilla/blindjoin/pull/3) when approved.
