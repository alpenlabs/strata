use alpen_vertex_state::chain_state::ChainState;
use alpen_vertex_state::state_op::WriteBatch;

use crate::define_table_with_seek_key_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;

// Consensus Output Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client state updates.
    (ChainStateSchema) u64 => ChainState
);

// Consensus State Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client states.
    (WriteBatchSchema) u64 => WriteBatch
);
