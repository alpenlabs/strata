use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;
use crate::traits::BlockStatus;
use alpen_vertex_state::block::L2Block;
use alpen_vertex_state::id::L2BlockId;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SomeBlock {
    name: String,
}

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block id to Block
    (L2BlockSchema) L2BlockId => L2Block
);

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block id to BlockStatus
    (L2BlockStatusSchema) L2BlockId => BlockStatus
);

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block id to BlockId
    (L2BlockHeightSchema) u64 => Vec<L2BlockId>
);
