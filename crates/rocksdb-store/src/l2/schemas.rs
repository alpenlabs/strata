use crate::define_table_with_default_codec;
use crate::define_table_with_seek_key_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;
use alpen_express_db::traits::BlockStatus;
use alpen_express_state::block::L2BlockBundle;
use alpen_express_state::id::L2BlockId;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SomeBlock {
    name: String,
}

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block id to Block
    (L2BlockSchema) L2BlockId => L2BlockBundle
);

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block id to BlockStatus
    (L2BlockStatusSchema) L2BlockId => BlockStatus
);

define_table_with_seek_key_codec!(
    /// A table to store L2 Block data. Maps block id to BlockId
    (L2BlockHeightSchema) u64 => Vec<L2BlockId>
);
