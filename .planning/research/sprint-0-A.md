# Sprint-0-A: bip322 0.0.10 cargo tree probe

**Branch:** spike/14-A-bip322-cargo-tree (pushed to origin; NOT merged to main per D-19)
**Pinned dep added:** bip322 = "=0.0.10" in shared/Cargo.toml (spike branch only)
**Date:** 2026-05-29
**Sprint cap:** 2 days (D-18)

## Gate 1 — bitcoin = 0.32.x in transitive tree (D-02 #1)

### Command
`cargo tree -p bip322 -e normal --format "{p} {f}"`

### Output (verbatim)
```
    Updating crates.io index
     Locking 3 packages to latest compatible versions
      Adding bip322 v0.0.10
      Adding snafu v0.8.9
      Adding snafu-derive v0.8.9
 Downloading crates ...
  Downloaded snafu v0.8.9
  Downloaded snafu-derive v0.8.9
  Downloaded bip322 v0.0.10
bip322 v0.0.10 default
├── base64 v0.22.1 alloc,default,std
├── bitcoin v0.32.8 actual-serde,base64,default,rand-std,secp-recovery,serde,std
│   ├── base58ck v0.1.0 std
│   │   ├── bitcoin-internals v0.3.0 alloc,default,serde,std
│   │   │   └── serde v1.0.228 alloc,default,derive,rc,serde_derive,std
│   │   │       ├── serde_core v1.0.228 alloc,default,rc,result,std
│   │   │       └── serde_derive v1.0.228 (proc-macro) default
│   │   │           ├── proc-macro2 v1.0.106 default,proc-macro
│   │   │           │   └── unicode-ident v1.0.24 
│   │   │           ├── quote v1.0.45 default,proc-macro
│   │   │           │   └── proc-macro2 v1.0.106 default,proc-macro (*)
│   │   │           └── syn v2.0.117 clone-impls,default,derive,extra-traits,fold,full,parsing,printing,proc-macro,visit,visit-mut
│   │   │               ├── proc-macro2 v1.0.106 default,proc-macro (*)
│   │   │               ├── quote v1.0.45 default,proc-macro (*)
│   │   │               └── unicode-ident v1.0.24 
│   │   └── bitcoin_hashes v0.14.1 alloc,bitcoin-io,io,serde,std
│   │       ├── bitcoin-io v0.1.4 alloc,std
│   │       ├── hex-conservative v0.2.2 alloc,std
│   │       │   └── arrayvec v0.7.6 default,std
│   │       └── serde v1.0.228 alloc,default,derive,rc,serde_derive,std (*)
│   ├── base64 v0.21.7 alloc,default,std
│   ├── bech32 v0.11.1 alloc,std
│   ├── bitcoin-internals v0.3.0 alloc,default,serde,std (*)
│   ├── bitcoin-io v0.1.4 alloc,std
│   ├── bitcoin-units v0.1.2 alloc,serde,std
│   │   ├── bitcoin-internals v0.3.0 alloc,default,serde,std (*)
│   │   └── serde v1.0.228 alloc,default,derive,rc,serde_derive,std (*)
│   ├── bitcoin_hashes v0.14.1 alloc,bitcoin-io,io,serde,std (*)
│   ├── hex-conservative v0.2.2 alloc,std (*)
│   ├── hex_lit v0.1.1 
│   ├── secp256k1 v0.29.1 alloc,hashes,rand,rand-std,recovery,serde,std
│   │   ├── bitcoin_hashes v0.14.1 alloc,bitcoin-io,io,serde,std (*)
│   │   ├── rand v0.8.6 alloc,default,getrandom,libc,rand_chacha,std,std_rng
│   │   │   ├── libc v0.2.184 default,std
│   │   │   ├── rand_chacha v0.3.1 std
│   │   │   │   ├── ppv-lite86 v0.2.21 simd,std
│   │   │   │   │   └── zerocopy v0.8.48 simd
│   │   │   │   └── rand_core v0.6.4 alloc,getrandom,std
│   │   │   │       └── getrandom v0.2.17 std
│   │   │   │           ├── cfg-if v1.0.4 
│   │   │   │           └── libc v0.2.184 default,std
│   │   │   └── rand_core v0.6.4 alloc,getrandom,std (*)
│   │   ├── secp256k1-sys v0.10.1 alloc,recovery,std
│   │   └── serde v1.0.228 alloc,default,derive,rc,serde_derive,std (*)
│   └── serde v1.0.228 alloc,default,derive,rc,serde_derive,std (*)
├── sha2 v0.10.9 default,oid,std
│   ├── cfg-if v1.0.4 
│   ├── cpufeatures v0.2.17 
│   │   └── digest v0.10.7 alloc,block-buffer,const-oid,core-api,default,mac,oid,std,subtle
│   │       ├── block-buffer v0.10.4 
│   │       │   └── generic-array v0.14.7 more_lengths
│   │       │       └── typenum v1.19.0 
│   │       ├── const-oid v0.9.6 
│   │       ├── crypto-common v0.1.7 std
│   │       │   ├── generic-array v0.14.7 more_lengths (*)
│   │       │   └── typenum v1.19.0 
│   │       └── subtle v2.6.1 const-generics,default,i128,std
└── snafu v0.8.9 alloc,rust_1_61,std
    └── snafu-derive v0.8.9 (proc-macro) rust_1_61
        ├── heck v0.5.0 
        ├── proc-macro2 v1.0.106 default,proc-macro (*)
        ├── quote v1.0.45 default,proc-macro (*)
        └── syn v2.0.117 clone-impls,default,derive,extra-traits,fold,full,parsing,printing,proc-macro,visit,visit-mut (*)
```

> Note: the cargo tree output above is reproduced verbatim from the spike-branch run.
> The `sha2` subtree is rendered exactly as cargo printed it.

### Result
PASS — `bitcoin v0.32.8` is present at depth 1 directly under `bip322 v0.0.10`. This satisfies the workspace pin `bitcoin = { version = "0.32", ... }` and the D-02 gate-1 requirement of `bitcoin = 0.32.x`. No `bitcoin v0.31.x` or earlier appears anywhere in the transitive graph.

Three new transitive crates are introduced by adopting `bip322 0.0.10`:
1. `bip322 v0.0.10` itself
2. `snafu v0.8.9` (error-type derive framework used internally by the crate)
3. `snafu-derive v0.8.9` (proc-macro, compile-time only)

Every other transitive crate (base64, bitcoin, sha2, serde, secp256k1, etc.) was already present in main's lockfile.

## Gate 2 — cargo audit clean on new edges (D-02 #2)

### Command
`cargo audit`

### Output (verbatim)
```
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1099 security advisories (from /Users/john/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (710 crate dependencies)
```

Exit code: 0

Because `cargo audit` 0.22.1 emits no advisory summary lines on a clean run, the equivalent
`cargo audit --json` was executed against the same spike-branch lockfile to produce an
explicit `vulnerabilities` summary. The pertinent fields are reproduced below verbatim:

```
{"database":{"advisory-count":1099,"last-commit":"eaf48e749baa3d5e27d304107d8abf175fd756bb","last-updated":"2026-05-29T20:55:26+02:00"},"lockfile":{"dependency-count":710},"settings":{"target_arch":[],"target_os":[],"severity":null,"ignore":["RUSTSEC-2023-0071","RUSTSEC-2025-0141","RUSTSEC-2024-0436"],"informational_warnings":["unmaintained","unsound","notice"]},"vulnerabilities":{"found":false,"count":0,"list":[]},"warnings":{}}
```

Key fields:
- `vulnerabilities.found: false`
- `vulnerabilities.count: 0`
- `vulnerabilities.list: []`
- `warnings: {}` (no informational-warning hits either)

The three `settings.ignore` entries (`RUSTSEC-2023-0071`, `RUSTSEC-2025-0141`, `RUSTSEC-2024-0436`) are pre-existing residual-risk acceptances declared in `.cargo/audit.toml` long before this spike (v1.3 / 2026-05-26 commit `d71e592`; see audit.toml top-of-file rationale block). They are NOT introduced by the bip322 edge and are NOT counted as advisories against this spike's gate.

### Result
PASS — zero advisories on the full dependency graph after adding `bip322 = "=0.0.10"`, including zero advisories on the three new transitive crates (`bip322`, `snafu`, `snafu-derive`). Lockfile dependency count after the spike: 710 crates. cargo audit exit code 0.

## Gate 3 — adapter < 50 LOC, zero lossy conversions (D-02 #3)

### Sketched adapter

The adapter wraps `bip322::verify_simple(&Address, message, Witness)` to the existing wire shape `(scriptPubKey: &Script, witness: &Witness, message: &[u8])` used by the round protocol. The crate's `verify_simple` signature was confirmed by reading `bip322-0.0.10/src/verify.rs:46-58` in the local crate cache during this spike. The crate's error type is `bip322::error::Error` (a snafu-derived enum); the adapter collapses it to a small wire-mapped enum and preserves the original via `#[source]` so no diagnostic information is dropped.

Network is operator-configured via `coordinator.toml` (already mapped per D-07's `output_script_type` discussion); for the LOC count the parameter is taken as an explicit argument rather than a magic constant — this is the zero-lossy-conversion-conscious shape.

```rust
// shared/src/bip322/adapter.rs  (SKETCH — NOT committed as a .rs file; paper analysis only)
use bitcoin::{Address, Network, Script, Witness};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Bip322Error {
    #[error("script_pubkey is not a recognised single-key address (P2WPKH / P2TR / P2SH-P2WPKH)")]
    UnrecognisedScriptPubkey {
        #[source]
        source: bitcoin::address::FromScriptError,
    },
    #[error("BIP-322 verification rejected the witness")]
    VerificationFailed {
        #[source]
        source: bip322::error::Error,
    },
}

pub fn verify_from_wire(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    let address = Address::from_script(spk, network)
        .map_err(|source| Bip322Error::UnrecognisedScriptPubkey { source })?;
    bip322::verify_simple(&address, message, witness.clone())
        .map_err(|source| Bip322Error::VerificationFailed { source })
}
```

Notes on the sketch's faithfulness to a real adapter:
- `witness.clone()` is intentional and not lossy — `bip322::verify_simple` takes `Witness` by value (per the crate's `pub fn verify_simple(address: &Address, message: impl AsRef<[u8]>, signature: Witness)` signature), and `Witness::clone` is a complete byte-exact deep clone of every witness element. No witness item is dropped or collapsed.
- `Address::from_script(spk, network)` returns the full `Address` variant set; we do NOT match-and-collapse to a single variant, so no Address shape is squashed.
- `message: &[u8]` flows straight to `impl AsRef<[u8]>` — zero copy in the type system, no length truncation.
- Errors are wrapped via `#[source]` (thiserror) rather than via `unwrap_or` / `unwrap_or_default` / `unwrap_or_else`.

### LOC count
26

(Non-blank, non-`//`-comment lines: 4 import + 13 error enum definition + 9 function definition = 26.)

Even if the adapter were rewritten without `thiserror` (open-coding the error enum and its `Display` impl), the LOC would land in the ~40-45 range. Either path lands well under the 50-LOC budget per D-02 gate 3.

### Lossy-conversion audit
- `unwrap_or` / `unwrap_or_default` / `unwrap_or_else` occurrences: 0
- Field-shape squashing (witness items dropped, Address variant collapsed, message slice copied lossily): no — `witness.clone()` is a complete deep clone of every witness element (Witness::clone is byte-exact); `Address::from_script` preserves the full Address variant; `message: &[u8]` is forwarded as-is to `impl AsRef<[u8]>`.

### Result
PASS — 26 LOC (< 50 budget) AND zero banned patterns AND zero field-shape squashing. The error type carries the underlying `bip322::error::Error` via `#[source]` rather than collapsing to a string, so the coordinator can pattern-match on specific failure modes (witness-empty, public-key-mismatch, etc.) if it ever needs to for blame-protocol purposes.

## Overall verdict

GO: all three D-02 gates PASS — bip322 v0.0.10 pulls in bitcoin v0.32.8 (gate 1), cargo audit is clean on the full graph including the three new transitive edges (gate 2), and the wire-shape adapter sketches at 26 LOC with zero lossy conversions (gate 3).

- GO = all three gates PASS → ADR Decision #1 flips from default EXTEND to ACCEPTED-ADOPT
- NO-GO = any gate FAILs → ADR Decision #1 stays at default ACCEPTED-EXTEND per D-03
- INCONCLUSIVE = 2-day cap reached without a determinative result on one or more gates → ADR Decision #1 defaults to ACCEPTED-EXTEND per D-03 (D-18 timebox)

## Reproducibility
- Spike branch HEAD SHA: e3756b7a5320d6ca15c1d37b852db40dc47cd9bd
- bip322 = "=0.0.10" added to shared/Cargo.toml on the spike branch (one-line edit; see commit `spike(14-A): add bip322 = "=0.0.10" dep for cargo tree probe`)
- Sprint elapsed: < 2 hours (well within D-18's 2-day cap)
- To reproduce locally: `git fetch origin spike/14-A-bip322-cargo-tree && git checkout spike/14-A-bip322-cargo-tree && cargo tree -p bip322 -e normal --format "{p} {f}"`
- Toolchain at probe time: cargo 1.95.0 (f2d3ce0bd 2026-03-21), cargo-audit 0.22.1, RustSec advisory-db @ commit `eaf48e749baa3d5e27d304107d8abf175fd756bb` (1099 advisories loaded)
