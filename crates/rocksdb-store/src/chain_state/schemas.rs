use strata_state::{id::L2BlockId, state_op::WriteBatch};

use crate::{define_table_with_seek_key_codec, define_table_without_codec, impl_borsh_value_codec};

// Consensus State Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client states.
    (WriteBatchSchema) u64 => WriteBatch
);

// L2 chain blocks as processed by chain state.
define_table_with_seek_key_codec!(
    /// Table to store known L2 Chain.
    (ChainSchema) u64 => L2BlockId
);
