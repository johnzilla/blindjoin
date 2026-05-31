# Phase 21: Audit Charter & Zeroization Tightening - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-31
**Phase:** 21-audit-charter-zeroization-tightening
**Areas discussed:** RoundSecretKey integration shape, Plan split + sequencing, Charter depth/voice/structure, audit.toml rationale style + Reviewed date timing

---

## RoundSecretKey integration shape

### Q1: Newtype shape — what wraps what, where it sits

| Option | Description | Selected |
|--------|-------------|----------|
| Inside RsaBlindSigner | Replace `secret_key: BjSecretKey` with `secret_key: RoundSecretKey` inside RsaBlindSigner. RoundStateInner.rsa_signer becomes `Option<RsaBlindSigner>`. Externals (`blind_sign`, `public_key_hash`) unchanged. | ✓ |
| Replace RsaBlindSigner field directly | RoundStateInner gains `signer: Option<RoundSecretKey>`; RsaBlindSigner stays only for keypair-generation/SPKI helpers. All blind-sign call sites change. | |
| Sibling field on RoundStateInner | Keep `rsa_signer: RsaBlindSigner` AND add `signer: Option<RoundSecretKey>` as a Drop-guarding sentinel. | |

**User's choice:** Inside RsaBlindSigner.
**Notes:** Minimal call-site delta; charter prose: "BjSecretKey held inside RoundSecretKey held inside RsaBlindSigner held inside Option<RsaBlindSigner>; setting Option to None triggers the Drop chain."

### Q2: Drop body shape

| Option | Description | Selected |
|--------|-------------|----------|
| Best-effort secure_erase + drop | RoundSecretKey::drop recovers DER bytes (or mutates inner fields if exposed) and zeroizes; existing Drop chain still fires. | ✓ |
| Structural-only (no in-place scrub) | RoundSecretKey::drop is no-op with doc-comment citing upstream limitation. | |
| Replace inner with garbage, then drop | `std::mem::replace` with a fresh keygen. Cost: 1 RSA-2048 keygen per round end. | |

**User's choice:** Best-effort secure_erase + drop.
**Notes:** Two layers structurally bounded by Option<RsaBlindSigner> lifetime.

### Q3: Drop trigger surface

| Option | Description | Selected |
|--------|-------------|----------|
| Keep existing trigger | All round end-states transition_to(Phase::Idle), which drops RoundStateInner → drops Option<RsaBlindSigner> → drops RoundSecretKey. No new explicit `signer = None` calls. | ✓ |
| Add explicit signer-drop helper | `pub fn drop_signer(&mut self) { self.rsa_signer = None; }`; call sites explicitly invoke before transition_to(Idle). | |
| Drop signer earlier (post-SIGNING) | Tighten window to Signing→Broadcast. Risk: code paths still referencing rsa_signer between Signing and Idle break. | |

**User's choice:** Keep existing trigger.
**Notes:** Verify all 4 valid Idle transitions reach transition_to(Idle) as planner audit task.

### Q4: Test pattern

| Option | Description | Selected |
|--------|-------------|----------|
| Structural + smoke RAM scan | (a) structural FSM test (load-bearing); (b) optional best-effort RAM scan marked ignore-able. | ✓ |
| Structural only | Skip the RAM scan; structural lifetime bound IS the load-bearing claim. | |
| Custom volatile-write + memcmp | `core::ptr::write_volatile` + unsafe slice scan; charter must defend the unsafe. | |

**User's choice:** Structural + smoke RAM scan.
**Notes:** Tests split between `rsa.rs::tests` (scrub) and `state.rs::tests` (structural FSM, mirroring `transition_to_idle_clears_inner` at state.rs:262).

---

## Plan split + sequencing

### Q1: How to split Phase 21 into plans

| Option | Description | Selected |
|--------|-------------|----------|
| Two plans: code-first, prose-second | 21-01 = AUDIT-03 newtype + Drop + tests + D-07 rewrite; 21-02 = AUDIT-01 charter + AUDIT-02 audit.toml + README link. | ✓ |
| One plan: single atomic commit | All three requirements in one PLAN. | |
| Three plans: one per requirement | 21-01 AUDIT-03; 21-02 AUDIT-01 charter; 21-03 AUDIT-02 audit.toml. | |

**User's choice:** Two plans: code-first, prose-second.
**Notes:** Newtype must land before charter prose can describe it shipped.

### Q2: Test location within 21-01

| Option | Description | Selected |
|--------|-------------|----------|
| Split: rsa.rs tests scrub, state.rs tests structural | Each test lives next to the thing it pins. | ✓ |
| All in rsa.rs::tests | Single test module owns both. | |
| All in state.rs::tests | FSM-side single test module. | |

**User's choice:** Split: rsa.rs tests the scrub, state.rs tests the structural drop.

### Q3: 21-02 commit shape

| Option | Description | Selected |
|--------|-------------|----------|
| Single commit | Charter + audit.toml + README link in ONE commit; keeps anchors atomic. | ✓ |
| Three atomic sub-commits | Charter → audit.toml → README, three commits. Cleaner git log but anchor-drift risk. | |

**User's choice:** Single commit.

### Q4: Wave structure

| Option | Description | Selected |
|--------|-------------|----------|
| Sequential: 21-01 wave 1 → 21-02 wave 2 | Charter prose cites 21-01 line numbers; sequential same shape as Phase 19. | ✓ |
| Parallel: 21-01 \|\| 21-02 | Possible if prose written first with placeholder refs, refreshed at end. | |

**User's choice:** Sequential.

---

## Charter depth/voice/structure

### Q1: Authoring voice for the 8 mandated sections

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid: tables for facts, narrative for threats | §1, §3, §6, §8 = tables; §2, §4, §5, §7 = short narrative (3-6 paragraphs each). | ✓ |
| All narrative | Every section multi-paragraph. | |
| All tables + bullets | Every section tabular/bulleted. | |

**User's choice:** Hybrid.

### Q2: Out-of-scope extension beyond REQUIREMENTS Tor+PKARR

| Option | Description | Selected |
|--------|-------------|----------|
| Extend to all 3rd-party crypto crates | arti-client, pkarr, blind-rsa-signatures, bip322=0.0.10, rust-bitcoin/secp256k1, bdk_wallet — each with per-line rationale. | ✓ |
| Stick to REQUIREMENTS verbatim (just Tor + PKARR) | Only the 2 mentioned. | |
| Mark out-of-scope but invite spot-checks | Extended set with hedged language. | |

**User's choice:** Extend to all 3rd-party crypto crates.

### Q3: §8 glossary scope

| Option | Description | Selected |
|--------|-------------|----------|
| Only active v1.4/v1.5 identifiers | ~25-30 entries; retired pre-v1.4 markers excluded. | ✓ |
| Every identifier ever used | 80+ entries including retired v1.0-1.3 markers. | |
| Active + 1-line forward refs to milestone archives | Active set + pointer to archives. | |

**User's choice:** Only active v1.4/v1.5 identifiers.

### Q4: §7 residual risks scope

| Option | Description | Selected |
|--------|-------------|----------|
| Advisory + non-advisory risks | (a) 3 cargo-audit-flagged; (b) protocol-level; (c) operational. | ✓ |
| Advisory-only | Only the 3 cargo-audit-flagged entries. | |
| Advisory + named carry-forwards | Advisories + 4 CARRY-* / B-03 / TEST-EXT-* items. | |

**User's choice:** Advisory + non-advisory risks (3 sub-buckets).

### Q5: Anchor style for in-scope refs

| Option | Description | Selected |
|--------|-------------|----------|
| File:symbol refs | `coordinator/src/blind/rsa.rs::RoundSecretKey` — stable across line shifts. | ✓ |
| File:line refs pinned to v1.5-ship commit | `coordinator/src/blind/rsa.rs:NN` with SHA pin. | |
| Both: symbol + pinned-line-at-tag | Symbol + parenthetical "approx. line NN at v1.5 tag". | |

**User's choice:** File:symbol refs.
**Notes:** Charter §1 table includes a 3rd column for orientation line at v1.5 tag.

---

## audit.toml rationale style + Reviewed date timing

### Q1: Charter-anchor citation style in TOML comments

| Option | Description | Selected |
|--------|-------------|----------|
| Bare relative path + anchor | `See docs/AUDIT-CHARTER.md#rsa-zeroization-window for...`. Plain text, no markdown syntax. | ✓ |
| Markdown link | `See [§RSA Zeroization Window](docs/AUDIT-CHARTER.md#rsa-zeroization-window)`. | |
| Section-number only | `See AUDIT-CHARTER.md §5`. §-numbers re-number if sections reordered. | |

**User's choice:** Bare relative path + anchor.

### Q2: Reviewed date

| Option | Description | Selected |
|--------|-------------|----------|
| Bump to Phase 21 commit date | Honest — review happened when 21-02 landed. | ✓ |
| Defer to /gsd-complete-milestone | Land with TBD-v1.5-ship placeholder. | |
| Bump to today (2026-05-31) | Discuss-phase date; pre-write. | |

**User's choice:** Bump to Phase 21 commit date.

### Q3: NEW advisory detection + decision

| Option | Description | Selected |
|--------|-------------|----------|
| Research subtask: cargo audit fresh, classify each | 21-RESEARCH.md task; planner decides per-advisory. | ✓ |
| Defer detection to executor | Plan says "run cargo audit at start of execution, decide then". | |
| Assume zero new advisories | Reactive ignores at PR time. | |

**User's choice:** Research subtask.

### Q4: TOML layout

| Option | Description | Selected |
|--------|-------------|----------|
| Keep flat layout, append anchor | Existing `[advisories]\nignore = [...]` plus closing-line anchors per ignore. | ✓ |
| Add header-comment with Reviewed log | Append-only review history at top of file. | |
| Per-ignore [advisories.ignore.RUSTSEC-X] table | Restructure to TOML tables; needs upstream cargo-audit verification. | |

**User's choice:** Keep flat layout.

---

## Claude's Discretion

- CD-46: Exact prose wording of D-07 comment rewrite (load-bearing contract pinned).
- CD-47: DER-roundtrip vs replace-with-dummy scrub choice (default: DER-roundtrip + tracing::debug! note; 21-RESEARCH informs).
- CD-48: Exact ignore-or-fix decisions for new advisories (default heuristics specified).
- CD-49: Charter section anchor naming (atomic landing prevents drift).
- CD-50: Best-effort RAM-scan test implementation (default: capture DER pre-drop, scan post-drop; mark ignore-able if non-portable).
- CD-51: Charter §4 (v=2 OwnershipProof PSBT handling) paragraph count (3-8; load-bearing items pinned).
- CD-52: README callout exact insertion line (after "Supply-chain hygiene" paragraph, around line 300).

## Deferred Ideas

- Upstream `Zeroize` impl on `blind-rsa-signatures::SecretKey` (closure of best-effort gap; v1.6+).
- `KeyScriptMismatch` Bip322Error variant (carried from Phase 19 §Deferred; v1.6+).
- Per-input variable `fee_share` (REQUIREMENTS Future requirements; v1.6+).
- TEST-EXT-01/02/03 cross-implementation differential fixtures (charter §7 documents the gap; v1.6+ closes).
- CARRY-TOR-UAT — Tor-mode verification harness (Phase 8 carry-forward; v1.6+).
- CARRY-REPAIR-01-PR — next external PR moment.
- B-03 dynamic fee estimation (pre-mainnet; charter §7 (c) cites).
- External pen-test engagement (REQUIREMENTS Out of Scope; separate milestone after v1.5 ships).
- `docs/AUDIT-CHARTER.md` versioning strategy (single-charter v1.5; v1.6+ revisits in-place vs versioned).
- Promote `RoundSecretKey` to `shared/` crate (coordinator-local in v1.5; v1.6+ if client needs ephemeral keys).
- `Reviewed:` log structure on audit.toml (single line in v1.5; v1.6+ if cumulative history becomes a compliance need).
- Per-ignore TOML table layout (REJECTED for v1.5; cargo-audit schema is flat list).
