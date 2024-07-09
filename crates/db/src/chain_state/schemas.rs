use alpen_vertex_state::chain_state::ChainState;
use alpen_vertex_state::state_op::WriteBatch;

use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;


define_table_with_default_codec!(
    /// Table to store ChainStateBatch 
    (ChainWriteBatchSchema) u64 => WriteBatch
);

define_table_with_default_codec!(
    /// Table to store chain states.
    (ChainStateSchema) u64 => ChainState
);


