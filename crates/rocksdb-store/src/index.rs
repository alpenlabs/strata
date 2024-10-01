use rockbound::{Schema, TransactionCtx, TransactionDBMarker};

use crate::{define_table_with_default_codec, define_table_without_codec, impl_borsh_value_codec};

define_table_with_default_codec!(
    /// A table to store L1 Txn data, maps block header hash to txns
    (IndexSchema) String => u64
);

/// Increment and get index for `Schema`.
/// This should only be used to get atomically auto-incrementing keys for tables inside
/// transactions.
/// `IndexSchema` should NEVER be used directly outside of this method.
/// index starts from 0
pub(crate) fn get_next_index<S: Schema, DB: TransactionDBMarker>(
    txn: &TransactionCtx<DB>,
) -> anyhow::Result<u64> {
    let index_key = S::COLUMN_FAMILY_NAME.to_string();
    let next_idx = txn
        .get_for_update::<IndexSchema>(&index_key)?
        .map(|last_idx| last_idx + 1)
        .unwrap_or(0);
    txn.put(&index_key, &next_idx)?;
    Ok(next_idx)
}

/// Increment and get index for `Schema`.
/// This should only be used to get atomically auto-incrementing keys for tables inside
/// transactions.
/// `IndexSchema` should NEVER be used directly outside of this method.
/// index starts from `default` and updated by `increment(last_idx)`
pub(crate) fn get_next_index_opts<S: Schema, DB: TransactionDBMarker>(
    txn: &TransactionCtx<DB>,
    increment: impl Fn(u64) -> u64,
    default: u64,
) -> anyhow::Result<u64> {
    let index_key = S::COLUMN_FAMILY_NAME.to_string();
    let next_idx = txn
        .get_for_update::<IndexSchema>(&index_key)?
        .map(increment)
        .unwrap_or(default);
    txn.put(&index_key, &next_idx)?;
    Ok(next_idx)
}
