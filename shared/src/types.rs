/// Unique identifier for a CoinJoin round.
pub type RoundId = uuid::Uuid;

/// Fixed CoinJoin denomination in satoshis.
pub struct Denomination(pub u64);

impl Denomination {
    /// Returns the denomination value in satoshis.
    pub fn sats(&self) -> u64 {
        self.0
    }
}
