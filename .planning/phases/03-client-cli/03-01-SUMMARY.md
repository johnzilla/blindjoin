---
phase: 03-client-cli
plan: 01
subsystem: wallet
tags: [bdk_wallet, bitcoin, psbt, bip84, bip39, descriptor, coinjoin, cli]

# Dependency graph
requires:
  - phase: 01-core-protocol
    provides: ClientWallet with WIF signing, integration test harness
  - phase: 02-blame-hardening
    provides: InputRegState, verify_and_sign, full round flow

provides:
  - BdkClientWallet backed by bdk_wallet 2.3 with BIP-84 descriptor HD derivation
  - from_wif compat constructor (integration tests unchanged)
  - from_descriptor constructor for CLI --descriptor flag
  - generate constructor with BIP-39 mnemonic, prints descriptors, writes descriptors.txt 0600
  - CLI flags: --descriptor, --utxo-address, --generate-wallet
  - check_psbt_denomination_outputs() in sign.rs refusing PSBT with < participants_registered denom outputs
  - participants_registered and denomination_sats fields in InputRegState
affects:
  - 03-client-cli/03-02 (adversarial integration tests — depends on verify_and_sign refusal behavior)
  - future phases using ClientWallet

# Tech tracking
tech-stack:
  added:
    - bdk_wallet 2.3 (keys-bip39 feature enabled)
    - bdk_wallet::signer::SignOptions for PSBT signing
    - bdk_wallet::keys::bip39::{Mnemonic, Language, WordCount} for wallet generation
  patterns:
    - peek_address(0) for address derivation in single-use CLI wallet (no &mut needed)
    - pub type ClientWallet = BdkClientWallet (zero-cost alias preserving all caller sites)
    - witness_utxo set on PSBT input before wallet.sign() for segwit signing
    - wif_key: Option<String> field for extracting signing key without descriptor parsing

key-files:
  created: []
  modified:
    - Cargo.toml (workspace — bdk_wallet 2.3 with keys-bip39 feature)
    - client/Cargo.toml (added bdk_wallet workspace dep)
    - client/src/wallet.rs (full rewrite: BdkClientWallet with 3 constructors)
    - client/src/config.rs (--descriptor, --utxo-address, --generate-wallet flags; make utxo/wif optional)
    - client/src/main.rs (routing to from_wif, from_descriptor, or generate based on flags)
    - client/src/round/mod.rs (InputRegState: +participants_registered, +denomination_sats)
    - client/src/round/input.rs (populate new InputRegState fields from InfoResponse)
    - client/src/round/sign.rs (check_psbt_denomination_outputs + 4 unit tests)

key-decisions:
  - "peek_address(0) instead of next_unused_address: single-use CLI wallet has no address reuse concern; avoids &mut self on coinjoin_output_address/change_address"
  - "wif_key: Option<String> stored on BdkClientWallet: avoids fragile descriptor string parsing to recover the WIF; secret_key_for_signing() remains clean for BIP-322 in input.rs"
  - "bdk_wallet::signer::SignOptions (not deprecated re-export): suppresses deprecation warning for PSBT signing"
  - "check_psbt_denomination_outputs extracted as public fn: testable independently of async HTTP calls"

patterns-established:
  - "Pattern 1: Three-constructor wallet factory (from_wif/from_descriptor/generate) with unified ClientWallet type alias"
  - "Pattern 2: InputRegState captures round parameters at registration time for later verification (participants_registered, denomination_sats)"

requirements-completed: [CLI-02, CLI-03, CLI-04]

# Metrics
duration: 35min
completed: 2026-04-07
---

# Phase 3 Plan 01: bdk_wallet 2.3 HD Wallet + CLI-04 Output Verification Summary

**bdk_wallet 2.3 descriptor wallet with BIP-39 mnemonic generation, BIP-84 HD derivation, and PSBT output-count anti-censorship check before signing**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-07T00:00:00Z
- **Completed:** 2026-04-07
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Replaced raw-WIF Phase 1 ClientWallet with BdkClientWallet backed by bdk_wallet 2.3 (from_wif/from_descriptor/generate constructors)
- Added --descriptor, --utxo-address, --generate-wallet CLI flags; wallet generation writes descriptors.txt at 0600 permissions (T-03-04)
- CLI-04: check_psbt_denomination_outputs() refuses to sign if PSBT has fewer denomination outputs than participants_registered
- All 3 integration tests pass unchanged; 4 new CLI-04 unit tests pass

## Task Commits

1. **Task 1: Refactor wallet.rs to bdk_wallet 2.3** - `431f401` (feat)
2. **Task 2: CLI-04 output count verification before signing** - `a4315f1` (feat)

## Files Created/Modified

- `Cargo.toml` - Added keys-bip39 feature to workspace bdk_wallet dependency
- `client/Cargo.toml` - Added bdk_wallet workspace dependency
- `client/src/wallet.rs` - Full rewrite: BdkClientWallet with from_wif/from_descriptor/generate; ClientWallet type alias
- `client/src/config.rs` - Added --descriptor, --utxo-address, --generate-wallet flags; made --utxo/--utxo-wif optional
- `client/src/main.rs` - Route to correct constructor; handle --generate-wallet early exit
- `client/src/round/mod.rs` - InputRegState: added participants_registered and denomination_sats fields
- `client/src/round/input.rs` - Populate new InputRegState fields from InfoResponse
- `client/src/round/sign.rs` - Added check_psbt_denomination_outputs() + 4 unit tests

## Decisions Made

- **peek_address(0) over next_unused_address:** Single-use CLI wallet has no address reuse concern; avoids requiring `&mut self` on coinjoin_output_address/change_address, keeping all callers unchanged.
- **wif_key: Option<String> on BdkClientWallet:** Storing WIF directly avoids fragile descriptor-string parsing to recover the key for BIP-322 signing in input.rs.
- **bdk_wallet::signer::SignOptions:** Used the non-deprecated path instead of the top-level re-export to suppress compiler warning.
- **check_psbt_denomination_outputs as public fn:** Extracted from the async verify_and_sign to be testable independently without HTTP mocking.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] blind_sign API mismatch in unit test helper**
- **Found during:** Task 2 (sign.rs unit test compilation)
- **Issue:** Test helper called `sk.blind_sign(&mut DefaultRng, &blind_message)` but blind-rsa-signatures 0.17 `blind_sign` takes only `blind_msg` (no RNG argument)
- **Fix:** Changed call to `sk.blind_sign(&blinding_result.blind_message)`; used explicit `BjKeyPair`/`BjSecretKey` type aliases to resolve ambiguous `KeyPair::generate` type inference
- **Files modified:** client/src/round/sign.rs
- **Verification:** `cargo test -p client` passes 4 unit tests
- **Committed in:** a4315f1 (Task 2 commit)

**2. [Rule 1 - Bug] bdk_wallet keys-bip39 feature not enabled**
- **Found during:** Task 1 (wallet.rs compilation)
- **Issue:** `bdk_wallet::keys::bip39` is gated behind `keys-bip39` feature, which was not listed in workspace Cargo.toml
- **Fix:** Added `features = ["keys-bip39"]` to workspace bdk_wallet dependency
- **Files modified:** Cargo.toml
- **Verification:** `cargo build -p client` succeeds
- **Committed in:** 431f401 (Task 1 commit)

**3. [Rule 1 - Bug] next_unused_address requires &mut self**
- **Found during:** Task 1 (wallet.rs compilation)
- **Issue:** `bdk_wallet::Wallet::next_unused_address` requires `&mut self`, incompatible with `&self` on coinjoin_output_address/change_address (all callers pass immutable reference)
- **Fix:** Used `peek_address(KeychainKind::External, 0)` / `peek_address(KeychainKind::Internal, 0)` instead — takes `&self`, appropriate for single-use CLI wallet with no address reuse concern
- **Files modified:** client/src/wallet.rs
- **Verification:** All callers compile unchanged; integration tests pass
- **Committed in:** 431f401 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 1 — API reality vs. plan assumptions)
**Impact on plan:** All three were compile-time API mismatches fixed inline. No behavioral change to the protocol. No scope creep.

## Issues Encountered

None beyond the three auto-fixed API mismatches above.

## Known Stubs

None — all three wallet constructors are fully wired. Address derivation and PSBT signing use bdk_wallet. The generate path uses a real BIP-39 mnemonic and BIP-84 derivation path.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes beyond what was specified in the plan's threat model.

## Self-Check

Files verified:
- `client/src/wallet.rs` — exists, contains BdkClientWallet, ClientWallet alias, 3 constructors
- `client/src/config.rs` — exists, contains generate_wallet, descriptor, utxo_address fields
- `client/src/round/sign.rs` — exists, contains check_psbt_denomination_outputs, 4 unit tests
- `client/src/round/mod.rs` — exists, contains participants_registered, denomination_sats

Commits verified:
- `431f401` — feat(03-01): refactor wallet.rs to bdk_wallet 2.3
- `a4315f1` — feat(03-01): CLI-04 output count verification

## Self-Check: PASSED

## Next Phase Readiness

- Plan 03-02 (adversarial integration tests) can proceed — verify_and_sign now has the CLI-04 check to test against
- BdkClientWallet API is stable and backward-compatible with all existing callers
- No blockers

---
*Phase: 03-client-cli*
*Completed: 2026-04-07*
