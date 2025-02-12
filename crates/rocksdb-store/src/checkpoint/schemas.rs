use strata_db::types::CheckpointEntry;

use crate::{define_table_with_seek_key_codec, define_table_without_codec, impl_borsh_value_codec};

define_table_with_seek_key_codec!(
    /// A table to store idx -> `CheckpointEntry` mapping
    (CheckpointSchema) u64 => CheckpointEntry
);
