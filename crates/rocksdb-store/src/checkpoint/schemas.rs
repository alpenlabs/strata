use alpen_express_db::types::BatchCommitmentEntry;

use crate::{define_table_with_default_codec, define_table_without_codec, impl_borsh_value_codec};

define_table_with_default_codec!(
    /// A table to store idx -> BatchCommitment mapping
    (BatchCommitmentSchema) u64 => BatchCommitmentEntry
);
