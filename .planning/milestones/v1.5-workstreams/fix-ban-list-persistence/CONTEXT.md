---
workstream: fix-ban-list-persistence
priority: P1
created: 2026-05-25
trigger: External code review — ban list file evaporates on every Docker restart
blocked_by: [fix-round-bootstrap]
---

# Context

## Why this exists
External code review found that the coordinator's UTXO ban list is not
persisted in the standard Docker deployment. Attackers can re-use banned
UTXOs after every container restart, defeating the abuse-prevention
mechanism that protects round signing.

## Root cause locations
- `coordinator/src/config.rs:34-35` — `ban_list_path` defaults to relative path `"ban_list.jsonl"`, which resolves to the container's working directory (likely `/app`), not a mounted volume.
- `docker/docker-compose.yml:53-56` — only `coordinator-keys:/app/keys` is mounted; no volume for ban list data.
- `coordinator/src/main.rs:75-76` — ban list loaded from the configured path on startup, so persistence depends entirely on the volume mount existing.

## Scope of fix
1. Add a `coordinator-data` named volume to `docker/docker-compose.yml`, mounted at `/app/data`.
2. Pick one of two paths (lean toward option B):
   - **A:** Change the config default in `coordinator/src/config.rs:34` from `"ban_list.jsonl"` to `"/app/data/ban_list.jsonl"`.
   - **B (recommended):** Keep the relative default for dev ergonomics, but set `BLINDJOIN_COORDINATOR__BAN_LIST_PATH=/app/data/ban_list.jsonl` as an env var in the docker-compose service definition. Cleaner separation between dev and Docker defaults.
3. Update any Dockerfile WORKDIR / VOLUME hints so `/app/data` exists.
4. Add a smoke test (probably a small bash script under `tests/` or `docker/`):
   - Bring up the stack
   - POST a ban entry (or trigger one via the test harness once Workstream A lands)
   - `docker compose restart coordinator`
   - Verify the ban entry persists across restart

## Entry
Recommend `/gsd-quick` — small, well-scoped fix.

## Dependencies
- **Blocked by `fix-round-bootstrap`** — the smoke test in step 4 needs a working coordinator round flow to trigger a real ban. Can start the docker-compose / config changes (steps 1-3) earlier, but defer the smoke test until A lands.
