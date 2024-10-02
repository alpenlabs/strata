//! Defines magic values for the bridge client in operator mode.
//!
//! Separating these out so that they can split into configurable items later if necessary.

/// The number of threads allocated in the [`ThreadPool`](threadpool::ThreadPool) for the database
/// I/O operations.
///
/// This is set to be the sum of:
///
/// * `2` for `duty` db so it can process duties in parallel.
/// * `1` for `duty_index` db as that gets read/updated per duty batch.
/// * `2` for `transaction` db so it can also process duties in parallel.
///
/// # NOTE:
///
/// At the moment, the threadpool is only used for channel-based db operations.
pub(super) const DB_THREAD_COUNT: usize = 2 + 1 + 2;
