/// Basis points conversion factor (10000 bps = 100%)
pub const BPS_CONVERSION_FACTOR: i32 = 10000;

pub const SIGNAL_CHANNEL_CAPACITY: usize = 1000;

pub const PROFIT_CHANGE_THRESHOLD_BPS: i32 = 5;

pub const SIGNAL_TTL_SECONDS: i64 = 300;

pub const MAX_DATA_LATENCY_MS: u64 = 500;

pub const MAX_HISTORY_POINTS: usize = 10;

pub const HISTORY_RETENTION_HOURS: i64 = 24;

pub const MAX_DISCOVERY_SYMBOLS: usize = 50;

pub const DISCOVERY_REFRESH_INTERVAL: u64 = 3600;

pub const DISCOVERY_EVENT_CHANNEL_CAPACITY: usize = 1000;
