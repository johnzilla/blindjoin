use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};
use shared::bip322::ScriptType;

/// Estimate the fee share per participant for a CoinJoin transaction.
///
/// Uses a linear vsize model: fixed overhead + per-input + per-output costs.
/// This is a single canonical definition used by both the `get_tx` handler
/// (for display) and `assemble_and_broadcast` (for PSBT construction) to
/// ensure both paths always produce matching fee values (WR-04).
///
/// Phase 20 Task 1 (transitional): the formula is hardcoded to P2WPKH-only
/// via `script_input_vbytes(P2wpkh)` / `script_output_vbytes(P2wpkh)`. Task 2
/// rewrites this to take `&BipConfig` and use the worst-case-across-allowed-set
/// formula per D-122. The numeric outcome of this transitional state is
/// byte-identical to the pre-Phase-20 formula `10 + n * 68 + n * 2 * 31`.
///
/// # Arguments
/// - `n`        — number of participants (inputs and outputs)
/// - `fee_rate` — sat/vbyte fee rate from coordinator config
pub fn estimate_fee_share(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 {
        return 0;
    }
    let estimated_vsize = 10
        + n * script_input_vbytes(ScriptType::P2wpkh)
        + n * 2 * script_output_vbytes(ScriptType::P2wpkh);
    (estimated_vsize * fee_rate) / n
}
