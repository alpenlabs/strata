/// Maximum number of blocks allowed in a proving range.
///
/// This constant serves as a safety limit to prevent proving tasks from processing
/// an excessively large number of blocks. If the number of blocks to prove exceeds
/// this limit, the task will be aborted early.
pub const MAX_PROVING_BLOCK_RANGE: u64 = 1024;
