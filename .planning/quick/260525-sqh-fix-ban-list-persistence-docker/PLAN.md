---
quick_id: 260525-sqh
slug: fix-ban-list-persistence-docker
workstream: fix-ban-list-persistence
created: 2026-05-26
status: in-progress
---

# Fix ban-list persistence in Docker deployment

## Why
`coordinator/src/config.rs:24` documents the ban file as `BLAME-05: persists ban records across coordinator restarts`. The Rust code honors that contract. The Docker deployment silently breaks it: no volume mount, relative default path resolves to the container's working dir, ban list evaporates on every `docker compose restart coordinator`.

Loaded ban list code path (already correct):
- `coordinator/src/run.rs:65-82` — loads from `cfg.coordinator.ban_file_path` on startup, graceful on missing file, warns and starts empty. Append path is via blame module functions (`coordinator/src/round/signing.rs:419-454`).

## Pre-flight (orchestrator-verified, hand to executor as facts)

1. **Field name is `ban_file_path`**, NOT `ban_list_path` (config.rs:26).
2. **Env var prefix:** `BLINDJOIN__` with double-underscore separator (config.rs:84-95). So the correct env var is `BLINDJOIN__COORDINATOR__BAN_FILE_PATH=/app/data/ban_list.jsonl`. Note: double underscore between `BLINDJOIN` and `COORDINATOR`, and between `COORDINATOR` and `BAN_FILE_PATH`.
3. **Existing volume pattern** in `docker/docker-compose.yml` for the coordinator service uses `coordinator-keys:/app/keys`. The Dockerfile (line 25) already does `RUN mkdir -p /app/keys` to ensure the dir exists. Mirror this pattern for `/app/data`.
4. **Coordinator container runs as root** — no USER directive in the Dockerfile. So volume mount ownership is not an issue, but `mkdir -p /app/data` is still required so the path exists before the coordinator process tries to write to it.
5. **CI fix from PR #2 is merged** — main now runs `cargo test --workspace --all-targets`. New Rust integration tests will run on CI without extra wiring.

## Scope

### Commit 1: `docker: add coordinator-data volume for ban list persistence`
- Edit `docker/docker-compose.yml`:
  - Add a second volume mount on the `coordinator` service: `- coordinator-data:/app/data`. Place it directly below the existing `coordinator-keys` line for visual symmetry.
  - Add a new env var to the coordinator service: `BLINDJOIN__COORDINATOR__BAN_FILE_PATH: "/app/data/ban_list.jsonl"`. Place near the `BLINDJOIN__DISCOVERY__PKARR_KEY_FILE` line for symmetry.
  - Add `coordinator-data:` to the bottom-level `volumes:` block with a one-line comment explaining purpose (mirror the comment style on `coordinator-keys`).
- Edit `docker/Dockerfile`:
  - In the `coordinator` target (around line 23-27), add `RUN mkdir -p /app/data` right next to the existing `RUN mkdir -p /app/keys`.

### Commit 2: `test: ban list persistence across coordinator restart`
- Add a Rust integration test at `tests/integration/ban_list_persistence.rs` that:
  - Creates a temp file path (e.g. `tempfile::NamedTempFile` or a `tempfile::tempdir` + custom name) for the ban file.
  - Builds a `CoordinatorConfig::with_defaults()` and overrides `ban_file_path` to the temp path.
  - Writes a synthetic ban entry directly via the blame module's append function (whatever public API exists in `coordinator/src/round/blame.rs` — look for an `append_*` function used by `on_signing_timeout`). If only test access is via crate-internal, the integration test may need to use the public API via the running coordinator OR drop down to writing the ban file format directly.
  - Calls `crate::round::blame::load_unexpired_entries(path, now)` (already used by run.rs:70) and asserts the synthetic ban entry is returned.
  - Bonus: drop the in-memory `BanList`, reload from file, assert the ban is still recognized via `BanList::is_banned()`.
  - This is a unit-level persistence test — it does NOT need the full coordinator binary or bitcoind. Mirrors the in-process path that `tests/integration/round_bootstrap.rs` demonstrates.
- Add the new module to `tests/integration/mod.rs`.

### Commit 3 (only if needed): `docker: ensure /app/data exists with correct ownership`
- Skip if commit 1 already covered the `mkdir -p /app/data` in the Dockerfile. This is a fallback slot if separate concerns warrant a split commit.

## Validation

- Branch: `fix-ban-list-persistence-docker` from `main` (currently `ca61348`).
- Push and open PR via `gh pr create`. Title: `fix: persist ban list across coordinator restarts in Docker`.
- Watch PR CI: all 4 jobs should pass, and the new `ban_list_persistence` integration test should appear in the `cargo test` output.
- Optional manual verification (only if it's quick and Docker is available):
  - `cd docker && docker compose up -d coordinator bitcoind`
  - Wait for healthy
  - Write a test ban entry by curling an appropriate endpoint OR by `docker exec coordinator sh -c 'echo "..." >> /app/data/ban_list.jsonl'`
  - `docker compose restart coordinator`
  - `docker exec coordinator cat /app/data/ban_list.jsonl` — verify entry survives
- Do NOT merge — return PR URL.

## Constraints
- Don't touch `.planning/` git history.
- Atomic commits.
- Match existing docker-compose.yml indentation and style exactly.
- If the blame module's append API is private and the test needs crate-internal access, expose it as `pub(crate)` only if absolutely required — otherwise drop down to writing the ban-list JSONL format directly in the test (this format is presumably one JSON object per line; verify by reading `coordinator/src/round/blame.rs`).

## Expected output
- PR URL.
- Confirmation that the new `ban_list_persistence` integration test passed on CI.
- One-line note on the test approach taken (public API vs file-format-direct).
- One-line note on whether any Dockerfile/source changes beyond commit 1 were needed.
