use crate::bitcoin::tx::{script_input_vbytes, MAX_STANDARD_OUTPUT_VBYTES};
use crate::config::BipConfig;

/// Worst-case pre-registration fee share estimate per participant.
///
/// Used at INPUT_REG time before the coordinator knows which script types will
/// actually register — MUST overestimate so `build_coinjoin_psbt`'s real per-
/// input requirement cannot exceed what `validate_utxo` already required from
/// each participant. Over-charges a P2WPKH input in a round where P2SH-P2WPKH
/// (91 vB) is allowed but not yet registered, and over-charges light outputs in
/// a round where a heavier output could register — acceptable, because the real
/// `fee_share` at PSBT build time is the load-bearing number a participant
/// actually pays (the excess simply returns to their change).
///
/// Pessimistic on all three dimensions the build can exceed a naive estimate:
/// - **inputs:** `max(script_input_vbytes across allowed_set())` — the heaviest
///   input the allowlist permits.
/// - **outputs:** `MAX_STANDARD_OUTPUT_VBYTES` (43) for every denomination AND
///   change output — the heaviest standard output an address can encode. The
///   coordinator does not enforce that a registered output matches
///   `output_script_type` (uniform-output is an accepted residual, see
///   AUDIT-CHARTER V1.4-MIN-02) and `build_coinjoin_psbt` prices each output
///   from its ACTUAL script, so the gate must assume the worst or a heavier
///   output reopens the H1 wedge window (N-1).
/// - **n:** callers pass `min_participants`; the share is non-increasing in n,
///   so the smallest finalizing round is the most expensive per participant.
///
/// **Privacy property:** both the input-side `max(...)` and the output-side
/// constant are uniform regardless of participant registration order — the
/// estimate never leaks which script types are currently registered.
///
/// # Arguments
/// - `bip_config` — operator-configured allowlist (output type no longer affects
///   the estimate: outputs are priced at the max standard size)
/// - `n`          — number of participants (inputs and outputs)
/// - `fee_rate`   — sat/vbyte fee rate from coordinator config
pub fn estimate_fee_share(bip_config: &BipConfig, n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 {
        return 0;
    }
    let worst_input_vb = bip_config
        .allowed_set()
        .map(script_input_vbytes)
        .max()
        .expect("BipConfig::validate ensures at least one allow_* flag is true");
    // N denomination + N change outputs, each at the heaviest standard size.
    let output_vb = MAX_STANDARD_OUTPUT_VBYTES;
    let estimated_vsize = 10 + worst_input_vb * n + output_vb * 2 * n;
    (estimated_vsize * fee_rate) / n
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::bip322::ScriptType;

    /// Helper to build a BipConfig with all 3 allow_* flags set as specified,
    /// and a configurable `output_script_type`. Mirrors the production
    /// `BipConfig::default` shape but skips serde deserialization.
    fn make_bip_config(
        p2wpkh: bool,
        p2tr: bool,
        p2sh_p2wpkh: bool,
        output: ScriptType,
    ) -> BipConfig {
        BipConfig {
            allow_p2wpkh: p2wpkh,
            allow_p2tr: p2tr,
            allow_p2sh_p2wpkh: p2sh_p2wpkh,
            output_script_type: output,
        }
    }

    /// FEE-02 unit cross-check: with all 3 script types allowed, worst_input_vb =
    /// max(68, 58, 91) = 91; output_vb = MAX_STANDARD_OUTPUT_VBYTES = 43; n=3;
    /// fee_rate=2:
    ///   estimated_vsize = 10 + 91*3 + 43*2*3 = 10 + 273 + 258 = 541
    ///   total_fee = 541 * 2 = 1082
    ///   fee_share = 1082 / 3 = 360 sats/participant
    #[test]
    fn worst_case_picks_max_allowed_input_vbyte() {
        let bip = make_bip_config(true, true, true, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 3, 2), 360);
    }

    /// N-1 fix: the gate prices outputs at MAX_STANDARD_OUTPUT_VBYTES, so it no
    /// longer depends on the configured `output_script_type` — a round that
    /// advertises P2WPKH outputs must gate identically to one advertising P2TR,
    /// because a client can register a heavier output than advertised regardless.
    #[test]
    fn gate_is_independent_of_output_type() {
        let w = make_bip_config(true, false, false, ScriptType::P2wpkh);
        let t = make_bip_config(true, false, false, ScriptType::P2tr);
        let s = make_bip_config(true, false, false, ScriptType::P2shP2wpkh);
        assert_eq!(estimate_fee_share(&w, 3, 2), estimate_fee_share(&t, 3, 2));
        assert_eq!(estimate_fee_share(&w, 3, 2), estimate_fee_share(&s, 3, 2));
    }

    /// Sanity: zero participants returns zero (early-return preserved).
    #[test]
    fn zero_participants_returns_zero() {
        let bip = make_bip_config(true, true, true, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 0, 2), 0);
    }

    /// H1 regression: the per-participant share is monotonically NON-INCREASING in
    /// n, so the estimate at `min_participants` is an upper bound for every larger
    /// finalizing round. The INPUT_REG gate MUST use this bound; using
    /// `max_participants` under-charges and lets a marginally-funded UTXO pass
    /// registration then fail `build_coinjoin_psbt` at signing (round wedge +
    /// mass wrongful ban). If this ever fails, the registration gate at
    /// handlers.rs is no longer pessimistic and H1 has regressed.
    #[test]
    fn share_is_monotonic_non_increasing_in_n() {
        let bip = make_bip_config(true, true, true, ScriptType::P2wpkh);
        let min = 3u32;
        let at_min = estimate_fee_share(&bip, min, 2);
        for n in min..=64 {
            assert!(
                estimate_fee_share(&bip, n, 2) <= at_min,
                "share at n={n} exceeded the min_participants={min} bound — \
                 registration gate would under-charge and wedge the round",
            );
        }
    }

    /// H1 n-pessimism (still holds under the output-pessimistic formula). P2WPKH-only,
    /// rate=2, output priced at 43:
    ///   @ min=3:  vsize = 10 + 68*3 + 43*6 = 472; fee_share = 944 / 3  = 314
    ///   @ max=20: vsize = 10 + 68*20 + 43*40 = 3090; fee_share = 6180 / 20 = 309
    /// The min bound (314) must be >= the larger-n estimate (309), so the gate armed
    /// at min_participants covers every finalizing round size.
    #[test]
    fn min_participants_estimate_is_the_pessimistic_bound() {
        let bip = make_bip_config(true, false, false, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 3, 2), 314, "registration gate @ min=3");
        assert_eq!(estimate_fee_share(&bip, 20, 2), 309, "larger round @ n=20");
        assert!(estimate_fee_share(&bip, 3, 2) >= estimate_fee_share(&bip, 20, 2));
    }

    /// N-1: the gate must stay STRICTLY ABOVE the all-P2WPKH v1.4 build baseline (266
    /// sats/participant at n=3, see tx.rs `fee_share_p2wpkh_only_matches_v14_baseline`),
    /// because `build_coinjoin_psbt` now prices outputs from their actual scripts and a
    /// client can register a P2TR (43 vB) output in a P2WPKH round. The 48-sat margin
    /// (314 vs 266 = (43-31) * 2 outputs * rate 2) is the reopened wedge window this
    /// gate closes — a lighter gate would let a UTXO pass registration then fail the
    /// build-time InsufficientFunds check.
    #[test]
    fn gate_exceeds_p2wpkh_build_baseline_to_cover_heavy_outputs() {
        let bip = make_bip_config(true, false, false, ScriptType::P2wpkh);
        const P2WPKH_BUILD_BASELINE: u64 = 266;
        assert_eq!(estimate_fee_share(&bip, 3, 2), 314);
        assert!(
            estimate_fee_share(&bip, 3, 2) >= P2WPKH_BUILD_BASELINE,
            "gate must cover the P2WPKH build baseline with margin for heavier outputs",
        );
    }
}
