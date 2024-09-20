use strata_state::{chain_state::ChainState, state_op::WriteBatch};

use crate::{define_table_with_seek_key_codec, define_table_without_codec, impl_borsh_value_codec};

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
