//! Constants used throughout the codebase.
//!
//! Centralizing magic numbers improves maintainability and discoverability.

/// Weight factor for converting multi-line ranges to a 1D size.
/// Lines are weighted more heavily than characters within a line.
pub const RANGE_SIZE_LINE_WEIGHT: u64 = 10000;

/// Maximum depth for resolving binding chains to prevent infinite loops.
pub const MAX_CHAIN_DEPTH: usize = 10;

/// Debounce interval for document change analysis (milliseconds).
pub const CHANGE_DEBOUNCE_MS: u64 = 300;

/// Heartbeat interval for background tasks (seconds).
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;
