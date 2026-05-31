use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};
use crate::config::BipConfig;

/// Worst-case pre-registration fee share estimate per participant.
///
/// Used at INPUT_REG time before the coordinator knows which script types will
/// actually register — MUST overestimate so `build_coinjoin_psbt`'s real per-
/// input weight cannot exceed what `validate_utxo` already required from each
/// participant. Over-charges a P2WPKH input in a round where P2SH-P2WPKH
/// (91 vB) is allowed but not yet registered — acceptable, because the real
/// `fee_share` at PSBT build time is the load-bearing number a participant
/// actually pays.
///
/// **Privacy property:** using `max(script_input_vbytes across allowed_set())`
/// is uniform regardless of participant registration order — never leaks which
/// script types are currently registered. Using anything narrower (e.g., the
/// most-recently-registered script type) would leak ordering information
/// across rounds.
///
/// Single canonical fee helper consumed by both the `get_tx` handler (display)
/// and `assemble_and_broadcast` (broadcast) — both PSBT paths MUST agree
/// byte-exactly (WR-04). Phase 20 only changes the formula inside this helper;
/// the WR-04 single-source-of-truth contract is preserved.
///
/// # Arguments
/// - `bip_config` — operator-configured allowlist + output script type
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
    let output_vb = script_output_vbytes(bip_config.output_script_type);
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

    /// FEE-02 unit cross-check: with all 3 script types allowed and output=P2WPKH,
    /// worst_input_vb = max(68, 58, 91) = 91; output_vb = 31; n=3; fee_rate=2:
    ///   estimated_vsize = 10 + 91*3 + 31*2*3 = 10 + 273 + 186 = 469
    ///   total_fee = 469 * 2 = 938
    ///   fee_share = 938 / 3 = 312 sats/participant
    #[test]
    fn worst_case_picks_max_allowed_input_vbyte() {
        let bip = make_bip_config(true, true, true, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 3, 2), 312);
    }

    /// Sanity: zero participants returns zero (early-return preserved).
    #[test]
    fn zero_participants_returns_zero() {
        let bip = make_bip_config(true, true, true, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 0, 2), 0);
    }

    /// P2WPKH-only allowed set with output=P2WPKH at n=3, fee_rate=2:
    ///   worst_input_vb = 68; output_vb = 31; vsize = 10 + 68*3 + 31*6 = 400
    ///   fee_share = 800 / 3 = 266 sats/participant
    /// (Matches the FEE-03(a) v1.4 baseline that build_coinjoin_psbt also produces.)
    #[test]
    fn p2wpkh_only_matches_v14_baseline() {
        let bip = make_bip_config(true, false, false, ScriptType::P2wpkh);
        assert_eq!(estimate_fee_share(&bip, 3, 2), 266);
    }
}
