// Number of prover workers to spawn
pub const NUM_PROVER_WORKERS: usize = 64;
pub const MAX_PARALLEL_PROVING_INSTANCES: usize = 25;

// Wait time in seconds for the prover manager loop
pub const PROVER_MANAGER_INTERVAL: u64 = 2;
pub const CHECKPOINT_POLL_INTERVAL: u64 = 5;
