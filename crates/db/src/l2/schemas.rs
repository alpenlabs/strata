use crate::define_table_with_default_codec;
use crate::impl_borsh_value_codec;
use crate::define_table_without_codec;
use alpen_vertex_state::block::L2Block;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;


#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct SomeBlock{
    name:String
}

define_table_with_default_codec!(
    /// A table to store L2 Block data. Maps block index to header
    (L2BlockSchema) u64 => L2Block
);