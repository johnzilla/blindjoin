//! Integration test binary crate root.
//!
//! This file has a dual role:
//!   1. Declares the individual integration test submodules below.
//!   2. Hosts shared fixtures consumed by those submodules: the
//!      `require_bitcoind!()` macro (env-var-gated bitcoind discovery),
//!      the `BitcoindGuard` RAII type (replaces the historical
//!      leak-the-Node-so-bitcoind-survives pattern), and the
//!      `bootstrap_regtest_bitcoind()` async helper (consolidates daemon
//!      bring-up + cookie extraction + 101-block mining into a single locus).
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

/// Bitcoind RPC credentials extracted from the regtest cookie file.
///
/// Canonical handoff struct between [`bootstrap_regtest_bitcoind`] and
/// consuming tests. `user` and `pass` come from the bitcoind cookie file
/// (via `Node::params::get_cookie_values`), NOT from any configured
/// credentials — corepc-node provisions a per-run cookie inside the
/// node's tempdir-backed datadir.
#[derive(Clone, Debug)]
pub struct RpcCreds {
    pub url: String,
    pub user: String,
    pub pass: String,
}

/// RAII guard owning the regtest `bitcoind` child process for a test's
/// lifetime (Phase 9 TEST-03 / TEST-04 fix; replaces the historical
/// leak-the-Node pattern that previously kept bitcoind alive by
/// suppressing its destructor).
///
/// **Invariant:** The test MUST hold this guard for its entire duration.
/// Dropping it (end-of-scope, early `return`, panic unwind) terminates
/// the daemon. The guard's `Drop` impl calls `node.stop()` (graceful RPC
/// shutdown) then lets `corepc_node::Node`'s own `Drop` run
/// `process.kill()` as a belt-and-suspenders fallback (verified at
/// `corepc-node-0.12.0/node/src/lib.rs:575-582`). This is the load-bearing
/// fix for the cargo-stdout-pipe hang documented in TODO.md.
///
/// The `Option<Node>` shape (rather than a bare `node: Node` field) is
/// the standard "drain in Drop" idiom: `drop(&mut self)` needs to consume
/// the `Node` so `n.stop()` can run against an owned value before the
/// inner `Node::Drop` runs. `Option::take()` is the canonical way to
/// move out of a `&mut self` field.
///
/// **Divergence from `coordinator/src/network/tor.rs::ConnectionGuard`:**
/// `ConnectionGuard` is the structural inspiration but uses an implicit
/// `Drop` (the inherent `OwnedSemaphorePermit::Drop` releases the
/// semaphore for free). `BitcoindGuard` requires an **explicit** `impl
/// Drop` because `node.stop()` is an RPC call we must initiate before
/// any process-tree cleanup — the corepc-node `Drop` only sends SIGKILL,
/// which bypasses bitcoind's clean shutdown path.
pub struct BitcoindGuard {
    node: Option<corepc_node::Node>,
}

impl BitcoindGuard {
    /// Wrap a started `Node` so it is shut down deterministically on drop.
    pub fn new(node: corepc_node::Node) -> Self {
        Self { node: Some(node) }
    }

    /// Borrow the inner `Node` for RPC work (mining additional blocks,
    /// fetching addresses, etc.).
    ///
    /// Panics if the guard has already been taken — a live guard never
    /// hits this branch, because the `Option::take()` only runs inside
    /// `Drop::drop`, which terminates the guard's lifetime.
    pub fn node(&self) -> &corepc_node::Node {
        self.node
            .as_ref()
            .expect("BitcoindGuard already taken; this can only happen after drop")
    }
}

impl Drop for BitcoindGuard {
    fn drop(&mut self) {
        if let Some(mut n) = self.node.take() {
            // CR-01: Offload the synchronously-blocking `n.stop()` (which
            // ultimately calls `std::process::Child::wait`) onto the tokio
            // blocking pool so we do NOT stall a runtime worker thread for
            // the duration of bitcoind shutdown. `#[tokio::test]` uses the
            // current-thread runtime by default — blocking the worker
            // freezes the entire executor.
            //
            // We cannot `.await` the join handle here (Drop is sync), so we
            // detach. On test teardown the runtime shutdown waits for
            // blocking-pool tasks to finish before the process exits,
            // which is the desired behavior. `Node::Drop` runs
            // `process.kill()` as belt-and-suspenders inside the closure
            // when `n` falls out of scope on the blocking pool.
            //
            // Outside a tokio runtime context (no current handle, e.g. a
            // sync helper that constructs a guard for cleanup), fall back
            // to a direct blocking stop().
            //
            // Do NOT panic inside drop: a panic during an unwinding drop
            // would abort the test process and lose the original test
            // panic message.
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn_blocking(move || {
                        // WR-01: surface graceful-stop failures via stderr
                        // so a shutdown-hang flake leaves a triage trail.
                        // We still fall through to Node::Drop's SIGKILL,
                        // which the corepc-node Drop impl runs when `n`
                        // exits scope on the blocking pool.
                        if let Err(e) = n.stop() {
                            eprintln!(
                                "BitcoindGuard: graceful stop failed ({e}); \
                                 relying on Node::Drop SIGKILL fallback"
                            );
                        }
                        // n drops here on the blocking pool; Node::Drop
                        // runs process.kill() as belt-and-suspenders.
                    });
                }
                Err(_) => {
                    // WR-01: same triage trail in the sync fallback path.
                    if let Err(e) = n.stop() {
                        eprintln!(
                            "BitcoindGuard: graceful stop failed ({e}); \
                             relying on Node::Drop SIGKILL fallback"
                        );
                    }
                    // n drops here; Node::Drop runs process.kill() as
                    // belt-and-suspenders.
                }
            }
        }
    }
}

/// Spin up a regtest `bitcoind`, mine 101 blocks, and return an
/// [`RpcCreds`] handle bound to a [`BitcoindGuard`] that owns the daemon
/// process for the caller's full scope.
///
/// **Single locus** for regtest bring-up across all bitcoind-dependent
/// integration tests (D-13 + D-14). Consumers in `full_round.rs`,
/// `rate_limiting.rs`, and `round_bootstrap.rs` (migrated in plans 09-03
/// / 09-04) replace their bespoke `Node::with_conf` + cookie-extract +
/// mine-101 blocks with a single call to this helper.
///
/// **Caller contract:** Hold the returned [`BitcoindGuard`] for the
/// test's full duration. Dropping it earlier kills bitcoind mid-test,
/// breaking subsequent RPC calls. The credentials in the returned
/// [`RpcCreds`] remain valid only while the guard is alive.
///
/// **Skip contract — call `require_bitcoind!()` first and forward the
/// resolved `exe` string.** This helper takes the bitcoind executable path
/// as a parameter rather than re-resolving it: the macro's `None => return`
/// expansion only works in a function returning `()`, and this helper
/// returns `(BitcoindGuard, RpcCreds)`. Forwarding the macro's return
/// value also collapses the two-source-of-truth divergence (WR-03):
/// previously this helper called `require_bitcoind_inner()` a SECOND time
/// and panicked with its own message if it disagreed with the macro,
/// which produced two operator-facing panic strings that could drift.
///
/// Canonical caller shape:
/// ```ignore
/// #[tokio::test]
/// async fn my_test() {
///     let exe = require_bitcoind!();                       // skip if missing
///     let (guard, creds) = bootstrap_regtest_bitcoind(exe).await;
///     // ... use creds; hold guard for the test's duration ...
/// }
/// ```
///
/// **Stdio handling (D-15, amended):** Sets `Conf::view_stdout = false`
/// (Stdio::null — the default, but set explicitly so a future Conf
/// initializer that flips the default doesn't silently re-introduce the
/// pipe-hang) AND passes `-printtoconsole=0` via `Conf::args` as
/// defense-in-depth. Bitcoind's child stdio never inherits cargo's pipe,
/// which is the load-bearing fix for the integration-suite hang documented
/// in TODO.md.
pub async fn bootstrap_regtest_bitcoind(exe: String) -> (BitcoindGuard, RpcCreds) {
    // corepc_node::Node::with_conf is synchronous; tokio::task::spawn_blocking
    // bridges it onto the async runtime. The returned Node is Send (its
    // fields — Child, Client (jsonrpc::Client), DataDir, ConnectParams —
    // are all Send-safe; verified RESEARCH.md Pitfall 4 + Assumption A1),
    // so the BitcoindGuard wrapping it crosses the .await boundary cleanly.
    tokio::task::spawn_blocking(move || {
        use bitcoin::Address;
        use corepc_node::{Conf, Node};

        let mut conf = Conf::default();
        conf.network = "regtest";
        // D-15: route child stdio to /dev/null so bitcoind never inherits
        // cargo's stdout pipe. This is the corepc-node 0.12 default; set
        // explicitly so a future default flip can't silently regress.
        conf.view_stdout = false;
        // D-15 defense-in-depth: ask bitcoind itself to suppress its
        // console output. Even if view_stdout=false is bypassed, the
        // daemon will not write to its inherited stdout.
        conf.args.push("-printtoconsole=0");
        // Conf::default() already includes "-fallbackfee=0.0001" but we
        // assert it explicitly so the test harness contract is self-evident
        // at the callsite (no need to read Conf::default's source).
        if !conf.args.iter().any(|a| a.starts_with("-fallbackfee=")) {
            conf.args.push("-fallbackfee=0.0001");
        }

        let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
        let cookie = node
            .params
            .get_cookie_values()
            .expect("read cookie file")
            .expect("parse cookie values");
        let rpc_url = node.rpc_url();
        let creds = RpcCreds {
            url: rpc_url,
            user: cookie.user.clone(),
            pass: cookie.password.clone(),
        };

        // Mine 101 blocks so coordinator::run's startup_health_check
        // (block_count > 0) passes when it boots against this RPC.
        let mine_addr: Address = node.client.new_address().expect("get new address");
        node.client
            .generate_to_address(101, &mine_addr)
            .expect("generate 101 blocks");

        (BitcoindGuard::new(node), creds)
    })
    .await
    .expect("regtest bootstrap spawn_blocking panicked")
}

/// UTXO funding result for an integration test.
///
/// Canonical handoff struct between [`fund_regtest`] and consuming tests.
/// Three P2WPKH UTXOs derived from hardcoded test WIFs (not wallet-owned),
/// each funded with `(denomination + 50_000 sats fee margin)` via the
/// regtest wallet's `sendtoaddress` + a 1-confirmation block.
///
/// **Why a separate handoff struct (rather than returning a tuple):** the
/// fields cross the `mod.rs` ↔ `full_round.rs` module boundary and are
/// named at multiple consumer sites; a struct keeps the call shape stable
/// even if the funding contract grows future fields (e.g. an additional
/// change-output outpoint for a 4-participant scenario).
#[derive(Clone, Debug)]
pub struct FundedSetup {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_pass: String,
    /// `(outpoint "txid:vout", value_sats)` per participant.
    pub utxos: [(String, u64); 3],
}

/// Spin up a regtest `bitcoind` (via [`bootstrap_regtest_bitcoind`]), derive
/// 3 P2WPKH addresses from hardcoded test WIFs, send `denomination + 50_000`
/// sats to each, mine 1 confirmation block, and locate each output's vout
/// via `get_raw_transaction_verbose` (wallet-agnostic — works against the
/// Bitcoin Core v30+ descriptor wallets where `listunspent` does NOT return
/// UTXOs paid to addresses the wallet does not own; see 10-RESEARCH.md
/// Pitfall 1).
///
/// **Single locus** (D-06 promotion) for the funded-regtest setup across
/// all integration tests that need 3 spendable test UTXOs. Consumers in
/// `full_round.rs` (and any future plan that needs the same funded setup)
/// call this once at the top of the test, hold the returned
/// [`BitcoindGuard`] for the test's full duration, and consume the
/// returned [`FundedSetup`] for coordinator + client orchestration.
///
/// **Caller contract:** Hold the returned [`BitcoindGuard`] for the
/// test's full duration. Dropping it earlier kills bitcoind mid-test,
/// breaking subsequent RPC calls. The credentials in the returned
/// [`FundedSetup`] remain valid only while the guard is alive.
///
/// **Schema note (descriptor-wallet gotcha):** Bitcoin Core v30 made
/// descriptor wallets mandatory. The wallet's `listunspent` only returns
/// UTXOs the wallet owns (i.e. derived from the wallet's own descriptors)
/// — UTXOs paid to externally-derived P2WPKH addresses (which the test
/// WIFs produce) are invisible to it. This helper instead reads each
/// funding tx directly via `corepc_node::Client::get_raw_transaction_verbose`,
/// returning a `GetRawTransactionVerbose` whose `.outputs:
/// Vec<RawTransactionOutput>` carries the vout index, value (BTC), and
/// `scriptPubKey` for each output. We match the recipient address against
/// `output.script_pubkey.address` and capture the `(outpoint, value_sats)`
/// pair. This pattern is wallet-agnostic and works identically against
/// legacy and descriptor wallets.
///
/// **Skip contract — call `require_bitcoind!()` first and forward the
/// resolved `exe` string.** This helper takes the bitcoind executable path
/// as a parameter rather than re-resolving it: the macro's `None => return`
/// expansion only works in a function returning `()`, and this helper
/// returns `(BitcoindGuard, FundedSetup)`. Forwarding the macro's return
/// value preserves the single-source-of-truth-per-test-invocation
/// invariant Phase 9 WR-03 established.
///
/// Canonical caller shape:
/// ```ignore
/// #[tokio::test]
/// async fn my_test() {
///     let exe = require_bitcoind!();                       // skip if missing
///     let (bitcoind_guard, setup) = fund_regtest(exe).await;
///     // ... use setup; hold bitcoind_guard for the test's full duration ...
///     let _bitcoind_guard = bitcoind_guard;
/// }
/// ```
pub async fn fund_regtest(exe: String) -> (BitcoindGuard, FundedSetup) {
    // Shared bootstrap: brings up bitcoind, sets stdio to /dev/null,
    // passes -printtoconsole=0, mines 101 blocks. Returns the bare guard.
    let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind(exe).await;

    let RpcCreds {
        url: rpc_url,
        user: rpc_user,
        pass: rpc_pass,
    } = creds;

    // Move the bare BitcoindGuard into the closure; return it from the
    // closure paired with the FundedSetup. This eliminates the
    // Arc<BitcoindGuard> + Arc::try_unwrap plumbing the file-private
    // version carried (10-RESEARCH.md Anti-Pattern — IN-02 from Phase 9).
    tokio::task::spawn_blocking(move || {
        use std::str::FromStr;

        use bitcoin::{
            secp256k1::Secp256k1, Address, Amount, CompressedPublicKey, Network, PrivateKey, Txid,
        };

        let node = bitcoind_guard.node();

        // Hardcoded regtest WIF keys — REGTEST ONLY, zero monetary value.
        let test_wifs = [
            "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
            "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
            "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
        ];
        let denomination: u64 = 100_000;
        let fund_sats: u64 = denomination + 50_000; // covers denomination + fee margin
        let fund_btc = Amount::from_sat(fund_sats);

        let secp = Secp256k1::new();

        // Derive P2WPKH addresses for each test key (regtest). These are
        // EXTERNAL to the bitcoind regtest wallet — list_unspent will not
        // see them on v30+ descriptor wallets (see Schema note above).
        let utxo_addresses: Vec<Address> = test_wifs
            .iter()
            .map(|wif| {
                let sk = PrivateKey::from_wif(wif).expect("valid test WIF");
                let raw_pk =
                    bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk.inner);
                let cpk = CompressedPublicKey(raw_pk);
                Address::p2wpkh(&cpk, Network::Regtest)
            })
            .collect();

        // bootstrap_regtest_bitcoind already mined 101 blocks. We only need
        // a fresh mining address for the post-funding confirmation block.
        let mine_addr: Address = node.client.new_address().expect("get new address");

        // Fund each test address via sendtoaddress.
        let mut funding_txids: Vec<String> = Vec::new();
        for addr in &utxo_addresses {
            let txid_result = node
                .client
                .send_to_address(addr, fund_btc)
                .expect("send_to_address failed");
            // SendToAddress is a newtype: SendToAddress(pub String).
            funding_txids.push(txid_result.0.clone());
        }

        // Wallet-agnostic vout discovery: read each funding tx directly
        // via get_raw_transaction_verbose, find the output whose
        // scriptPubKey matches the intended recipient address. Works for
        // both legacy and descriptor wallets (10-RESEARCH.md Pattern 1 /
        // Example 4). Does NOT depend on wallet ownership of the
        // recipient address.
        //
        // ORDERING: this lookup MUST run BEFORE the confirmation-block
        // mine_addr generate_to_address call below. Bitcoin Core v30+
        // without `-txindex=1` cannot resolve a txid via
        // get_raw_transaction once the tx is buried in a block; the
        // mempool-resident form still resolves by txid alone. Reading
        // here while the funding txs are still in the mempool keeps the
        // helper wallet-agnostic AND txindex-agnostic.
        let utxos_vec: Vec<(String, u64)> = funding_txids
            .iter()
            .zip(utxo_addresses.iter())
            .map(|(funding_txid_str, recipient_addr)| {
                let txid =
                    Txid::from_str(funding_txid_str).expect("valid funding txid hex");
                let tx = node
                    .client
                    .get_raw_transaction_verbose(txid)
                    .expect("get_raw_transaction_verbose");

                let recipient_str = recipient_addr.to_string();
                // ScriptPubKey carries an `address: Option<String>` field on v23+
                // types (re-exported through v30 at feature 30_2). The expected
                // happy path matches via that field; this is the wallet-agnostic
                // equivalent of the broken list_unspent + address-string filter.
                let out = tx
                    .outputs
                    .iter()
                    .find(|o| {
                        o.script_pubkey.address.as_deref() == Some(&recipient_str)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "funding tx {} has no output to {}",
                            funding_txid_str, recipient_str
                        )
                    });

                let outpoint = format!("{}:{}", funding_txid_str, out.index);
                let value_sats = (out.value * 100_000_000.0).round() as u64;
                (outpoint, value_sats)
            })
            .collect();

        assert_eq!(utxos_vec.len(), 3, "must have 3 funded UTXOs");

        // Mine 1 confirmation block AFTER the vout reads above. The
        // FundedSetup outpoints are now confirmed UTXOs ready for the
        // coinjoin round.
        node.client
            .generate_to_address(1, &mine_addr)
            .expect("generate confirmation block");

        let setup = FundedSetup {
            rpc_url,
            rpc_user,
            rpc_pass,
            utxos: [
                utxos_vec[0].clone(),
                utxos_vec[1].clone(),
                utxos_vec[2].clone(),
            ],
        };

        (bitcoind_guard, setup)
    })
    .await
    .expect("fund_regtest spawn_blocking panicked")
}
