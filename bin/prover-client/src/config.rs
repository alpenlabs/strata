// Number of prover workers to spawn
pub const NUM_PROVER_WORKER: usize = 10;

// Wait time in seconds for the prover manager loop, in seconds
pub const PROVER_MANAGER_WAIT_TIME: u64 = 5;

// Interval between dispatching L2 block proving tasks, in seconds
pub const L2_BLOCK_PROVING_TASK_DISPATCH_INTERVAL: u64 = 1;

// Interval between dispatching BTC block proving tasks, in seconds
pub const BTC_BLOCK_PROVING_TASK_DISPATCH_INTERVAL: u64 = 10;

// Starting block height for EL block proving tasks
pub const EL_START_BLOCK_HEIGHT: u64 = 1;
pub const CL_START_BLOCK_HEIGHT: u64 = 1;
