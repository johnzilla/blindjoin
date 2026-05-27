//! Integration test binary crate root.
//!
//! This file has a dual role:
//!   1. Declares the individual integration test submodules below.
//!   2. Hosts shared fixtures consumed by those submodules: the
//!      `require_bitcoind!()` macro (env-var-gated bitcoind discovery),
//!      the `BitcoindGuard` RAII type (replaces the historical
//!      `Box::leak(node)` pattern), and the `bootstrap_regtest_bitcoind()`
//!      async helper (consolidates daemon bring-up + cookie extraction +
//!      101-block mining into a single locus).
//!
//! Why fixtures live here: `coordinator/Cargo.toml` declares
//! `[[test]] name = "integration" path = "../tests/integration/mod.rs"` at
//! lines 71-73, which makes THIS file the crate root of the integration test
//! binary. `#[macro_export]` macros defined here are therefore reachable as
//! `crate::require_bitcoind!()` from each `mod X;` submodule below (and as
//! `$crate::require_bitcoind!()` inside the macro itself).

mod ban_list_persistence;
mod full_round;
mod rate_limiting;
mod round_bootstrap;

/// Inner bitcoind-discovery accessor — returns `Some(path)` when the daemon
/// is available, `None` when it is not but the test should skip.
///
/// Behavior:
/// * `corepc_node::exe_path()` succeeds → returns `Some(path)`.
/// * `corepc_node::exe_path()` errors AND env `BLINDJOIN_REQUIRE_BITCOIND=1` →
///   panics with a message naming both `BLINDJOIN_REQUIRE_BITCOIND` and
///   `BITCOIND_EXE` so a failing CI log triages without re-reading source.
/// * `corepc_node::exe_path()` errors AND env `BLINDJOIN_REQUIRE_BITCOIND`
///   is unset or any value other than `"1"` → emits an `eprintln!` notice
///   and returns `None`.
///
/// Tests should prefer the `require_bitcoind!()` macro over calling this
/// directly: a bare function cannot `return` from the calling test, so a
/// `let exe = require_bitcoind_inner().expect(...)` would either run the test
/// against a missing daemon (wrong) or call `std::process::exit(0)` which
/// would abort the entire test binary, masking sibling failures (RESEARCH.md
/// Pattern 2 "Important footgun"). The macro returns from the calling test
/// function's scope, which is the correct skip semantic.
///
/// The env-var idiom matches the project canonical form at
/// `coordinator/src/run.rs:296-298`
/// (`std::env::var("BLINDJOIN_*").map(|v| v == "1").unwrap_or(false)`),
/// using the equivalent `as_deref() == Ok("1")` spelling.
pub fn require_bitcoind_inner() -> Option<String> {
    match corepc_node::exe_path() {
        Ok(p) => Some(p),
        Err(e) => {
            if std::env::var("BLINDJOIN_REQUIRE_BITCOIND").as_deref() == Ok("1") {
                panic!(
                    "bitcoind required but not found ({e}). \
                     BLINDJOIN_REQUIRE_BITCOIND=1 is set — this is CI mode. \
                     Check that BITCOIND_EXE points to a valid binary."
                );
            }
            eprintln!(
                "bitcoind not found ({e}), skipping (local-dev mode; \
                 set BLINDJOIN_REQUIRE_BITCOIND=1 to fail instead)"
            );
            None
        }
    }
}

/// Resolve the bitcoind binary path or `return` from the calling test.
///
/// Use as the first line of any test that needs bitcoind:
/// ```ignore
/// #[tokio::test]
/// async fn my_test() {
///     let exe = require_bitcoind!();
///     // ... use exe ...
/// }
/// ```
///
/// Expands to:
/// ```ignore
/// match $crate::require_bitcoind_inner() {
///     Some(p) => p,
///     None => return,
/// }
/// ```
///
/// The macro form (vs. a plain function) is load-bearing: the `return`
/// expanded inside the test body exits ONLY the calling test, not the entire
/// test binary. A `pub fn require_bitcoind() -> String` form would have to
/// either `panic!` on miss (breaks local-dev opt-in) or `std::process::exit`
/// (aborts the whole binary). See RESEARCH.md Pattern 2 and Assumption A5.
///
/// Note on the unused-variable warning: callers that delegate daemon bring-up
/// to `bootstrap_regtest_bitcoind()` (which itself invokes `require_bitcoind!()`)
/// may not use the resolved path. `let _exe = require_bitcoind!();` or the
/// `_ = require_bitcoind!();` form is acceptable in that case; both still
/// route the skip path through `return`.
#[macro_export]
macro_rules! require_bitcoind {
    () => {
        match $crate::require_bitcoind_inner() {
            Some(p) => p,
            None => return,
        }
    };
}
