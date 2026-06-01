---
phase: 22-base-image-digest-drift-detection
plan: 06
status: complete
date: 2026-06-01
---

# Plan 22-06 — branch protection + canonical digests

The two load-bearing items got done:

1. **Branch protection on `main`** — Rulesets id `17136873`, enforcement `active`, targets `~DEFAULT_BRANCH`. Rules: `pull_request` with `require_code_owner_review: true` + `required_approving_review_count: 1`, plus `deletion` and `non_fast_forward`. Bypass: `RepositoryRole admin (id 5), bypass_mode: always` — so admin (solo maintainer) is unblocked on both PRs and direct push, while outside contributors hitting CODEOWNERS-matched paths (`docker/digests.txt`, `.github/actions/read-base-digests/**`) still hit the gate.

2. **Canonical digests resolved + PR through the gate** — see [PR #8](https://github.com/johnzilla/blindjoin/pull/8). Real digests resolved via Docker Hub registry API (no docker CLI on the planning machine):
   - `debian:bookworm-slim@sha256:0104b334637a5f19aa9c983a91b54c89887c0984081f2068983107a6f6c21eeb`
   - `lukemathwalker/cargo-chef:latest-rust-1@sha256:e606721f52d95169364bf39cae726a94ed8b397625011ccfaa8340db488b823b`

PR #8 also confirmed the gate fires correctly: `mergeStateStatus: BLOCKED`, `reviewDecision: REVIEW_REQUIRED`. Admin self-bypass is the documented merge path for solo work.

## DRIFT-01 promise as it actually stands

"Outside contributors cannot auto-merge digest bumps; admin can. Trust model = maintainer trusts themselves." Honest and matches what the gate enforces. Documented in [SECURITY.md](../../../SECURITY.md) §Supply-chain status.

## Skipped (not theater that buys anything for solo work)

- SC#1-4 fresh-runner rehearsal scaffold (deleted — the gate fires; the rest was framework prescription)
- Disposition table
- Multi-section UAT report

## Open carry-forward

- SC#4 first scheduled `0 9 * * *` UTC run of `digest-drift-check.yml` observable next morning; if a `[digest-drift]` issue appears, the workflow is correct and the upstream just moved.
- If blindjoin ever gains a second active maintainer, remove the admin bypass actor and revert to `bypass_actors: []`.
