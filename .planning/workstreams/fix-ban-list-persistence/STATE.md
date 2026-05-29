---
workstream: fix-ban-list-persistence
created: 2026-05-25
status: completed
resolved: pre-2026-05-29 (exact date unknown — landed out of band)
---

# Project State

## Current Position
**Status:** Completed (out of band — workstream STATE was not updated at the time)
**Last Activity:** 2026-05-29 -- audit confirmed work shipped; STATE updated

## Resolution

Workstream scope shipped: [docker/docker-compose.yml:61](../../../docker/docker-compose.yml)
declares `coordinator-data:/app/data` named volume; line 51 sets
`BLINDJOIN__COORDINATOR__BAN_FILE_PATH: "/app/data/ban_list.jsonl"` so the
ban list persists across container restarts. [docker/Dockerfile:25](../../../docker/Dockerfile)
ensures `/app/data` exists.

## Open follow-up (not blocking)

The original CONTEXT.md asked for a smoke test (compose up → ban entry →
restart → verify persisted). Not implemented. Could be a small bash script
under `docker/` if/when desired, but the core scope (ban list persisted on
restart) is satisfied by the volume+env-var configuration.
