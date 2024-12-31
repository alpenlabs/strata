//! Common constants for the operator binary in both the operator and challenger modes.

pub(super) const DEFAULT_RPC_PORT: u32 = 4781; // first 4 digits in the sha256 of "operator"

pub(super) const DEFAULT_RPC_HOST: &str = "127.0.0.1";

pub(super) const DEFAULT_DUTY_TIMEOUT_SEC: u64 = 600;
/// The default bridge rocksdb database retry count, if not overridden by the user.
pub(super) const ROCKSDB_RETRY_COUNT: u16 = 3;
