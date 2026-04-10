/// Estimate the fee share per participant for a CoinJoin transaction.
///
/// Uses a linear vsize model: fixed overhead + per-input + per-output costs.
/// This is a single canonical definition used by both the `get_tx` handler
/// (for display) and `assemble_and_broadcast` (for PSBT construction) to
/// ensure both paths always produce matching fee values (WR-04).
///
/// # Arguments
/// - `n`        — number of participants (inputs and outputs)
/// - `fee_rate` — sat/vbyte fee rate from coordinator config
pub fn estimate_fee_share(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 {
        return 0;
    }
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    (estimated_vsize * fee_rate) / n
}
