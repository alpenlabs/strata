// Number of prover workers to spawn
pub const NUM_PROVER_WORKERS: usize = 10;

// Wait time in seconds for the prover manager loop
pub const PROVER_MANAGER_INTERVAL: u64 = 5;

// Dispatch intervals and starting blocks for BTC and L2 (both EL & CL) proving tasks
pub const BTC_DISPATCH_INTERVAL: u64 = 10;
pub const BTC_START_BLOCK: u64 = 1;

pub const L2_DISPATCH_INTERVAL: u64 = 1;
pub const L2_START_BLOCK: u64 = 1;

pub const L1_BATCH_DISPATCH_INTERVAL: u64 = 60;
