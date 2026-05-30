# BIP-322 test vector vendor provenance

Per Phase 15 CONTEXT D-33: vendored snapshots with explicit commit SHA + capture date.
The JSON files are themselves bit-identical to their upstream sources; provenance
metadata lives here so the JSON parsers stay strict.

## basic-test-vectors.json

Source: https://raw.githubusercontent.com/bitcoin/bips/d77863fb9e/bip-0322/basic-test-vectors.json
Upstream commit SHA: `d77863fb9e` (May 2026 — "BIP-0322: update test vectors")
Captured: 2026-05-30
Fetch command:
```sh
curl -L "https://raw.githubusercontent.com/bitcoin/bips/d77863fb9e/bip-0322/basic-test-vectors.json" \
  > shared/tests/fixtures/bip322/basic-test-vectors.json
```

Contents (4 `simple` entries):
- `simple[0]`: P2WPKH (`bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l`) — empty message.
- `simple[1]`: P2WPKH (`bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l`) — message `"Hello World"`.
- `simple[2]`: P2WSH-multisig-3of3 (`bc1qp0ahvfh83088w49k405szqgg4f3pptr7p2g06tdxfjcd40z4lh4q95lsz9`) —
  out of v1.4 scope (RESEARCH Pitfall 6). The harness in `shared/tests/per_script_vectors.rs`
  classifies this entry as unsupported and skips it.
- `simple[3]`: P2TR (`bc1pss0zhytly75awhm6x2hhvd5lnzv3vssgrf9axfheq8ldyzn88ges79fler`) — message
  `"No prefix fallback"`. The per_script_vectors harness asserts this entry's witness verifies via
  `shared::bip322::verify_simple(ScriptType::P2tr, ...)`.

### Note on the May 2026 upstream encoding change (executor finding, 2026-05-30)

The May 2026 update (`d77863fb9e`) introduced a 3-byte `0xb2 0x6a 0x40` prefix on every P2WPKH
`bip322_signatures` entry — the resulting base64 strings have length 148 (`4n`) but decoded
bytes do NOT parse as a canonical `bitcoin::Witness` (the leading varint `0xb2 = 178` would
imply 178 witness elements, which fails consensus decoding). The bip322 = "=0.0.10" crate's
`verify_simple_encoded` (which our adapter wraps) rejects these strings as malformed.

We vendor the latest SHA verbatim per D-33 (single source of truth, no in-file edits), but the
per-script test harness defensively skips entries whose `bip322_signatures` fail base64-decode
into a canonical `Witness`. Clean P2WPKH vectors live in `p2sh_p2wpkh_supplement.json` (lifted
from the earlier `3ab70c98a7` upstream commit, where the canonical encoding was used). The clean
P2TR vector at `simple[3]` of `basic-test-vectors.json` decodes cleanly even though it was added
in the same May 2026 commit — its encoding does not carry the malformed prefix.

Upstream re-evaluation trigger: if a future BIP-322 file commit either documents the May 2026
prefix as an intentional encoding format (in the mediawiki spec) OR reverts the prefix, re-pin
SHA forward and remove the supplement's P2WPKH entries. This is a v1.5 TEST-EXT-01 candidate.

## p2sh_p2wpkh_supplement.json

Reason: upstream `basic-test-vectors.json` at `d77863fb9e` contains 0 P2SH-P2WPKH vectors
(RESEARCH Pitfall 6) AND the P2WPKH entries have malformed encoding (see note above). The
supplement provides:

### Entries 0-1 — P2WPKH (canonical encoding fallback from `3ab70c98a7`)

Source: https://raw.githubusercontent.com/bitcoin/bips/3ab70c98a7/bip-0322/basic-test-vectors.json
(the April 2026 commit — "BIP-0322: turn test vectors into JSON, add more"). The two P2WPKH
entries are lifted verbatim from `simple[0]` and `simple[1]` of that earlier upstream snapshot,
preserving the canonical `Witness`-consensus-encoded base64 form that the bip322 crate accepts.

### Entry 2 — P2SH-P2WPKH ("Hello World")

Source: `bip322` crate v0.0.10, `src/lib.rs` lines 46-48 (constants `NESTED_SEGWIT_ADDRESS` +
`NESTED_SEGWIT_WIF_PRIVATE_KEY`) and lines 299-321 (the `simple_sign_p2sh_p2wpkh` and
`roundtrip_p2sh_p2wpkh_simple` tests). The base64 signature is lifted verbatim from the
crate's `simple_sign_p2sh_p2wpkh` test (`src/lib.rs:300-304`) — it is the canonical output of
`bip322::sign_simple_encoded(NESTED_SEGWIT_ADDRESS, "Hello World", NESTED_SEGWIT_WIF_PRIVATE_KEY)`,
verified by the crate's own test suite.

Captured: 2026-05-30
Fetch commands (for forensic reproducibility — not run in CI):
```sh
# P2WPKH entries — lifted from earlier upstream commit
curl -L "https://raw.githubusercontent.com/bitcoin/bips/3ab70c98a7/bip-0322/basic-test-vectors.json" \
  | jq '[.simple[0], .simple[1]]'

# P2SH-P2WPKH entry — lifted from bip322 crate v0.0.10 test constants
# (no curl; see ~/.cargo/registry/src/index.crates.io-*/bip322-0.0.10/src/lib.rs:46-48 + lib.rs:299-321)
```

Contents:
- `[0]`: P2WPKH (`bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l`) — empty message
- `[1]`: P2WPKH (`bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l`) — message `"Hello World"`
- `[2]`: P2SH-P2WPKH (`3HSVzEhCFuH9Z3wvoWTexy7BMVVp3PjS6f`) — message `"Hello World"`

## Future work — TEST-EXT-01 (v1.5 candidate)

When the upstream BIP-322 file's encoding format stabilises and is documented in the mediawiki
spec, this supplement should be promoted to a cross-implementation differential test set sourced
from `bip322-js` (the ACken2/bip322-js reference TypeScript implementation). The supplement file
is the v1.4 minimum P2WPKH+P2TR+P2SH-P2WPKH coverage; the full differential matrix is v1.5 scope.
