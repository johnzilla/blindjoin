/// Client library for CoinJoin round participation.
///
/// Exposes wallet, HTTP client, and round modules for use in integration tests
/// and as a library dependency. The binary entry point is in main.rs.
pub mod config;
pub mod discover;
pub mod http;
pub mod round;
pub mod wallet;
