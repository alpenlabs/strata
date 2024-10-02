//! Common constants for the operator binary in both the operator and challenger modes.

pub(super) const RPC_PORT: usize = 4781; // first 4 digits in the sha256 of "operator"

pub(super) const RPC_SERVER: &str = "127.0.0.1";

/// The default bridge rocksdb database retry count, if not overridden by the user.
pub(super) const ROCKSDB_RETRY_COUNT: u16 = 3;
