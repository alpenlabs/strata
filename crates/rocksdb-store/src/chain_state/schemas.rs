use strata_state::{chain_state::Chainstate, state_op::WriteBatch};

use crate::{define_table_with_seek_key_codec, define_table_without_codec, impl_borsh_value_codec};

// Consensus Output Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client state updates.
    (ChainstateSchema) u64 => Chainstate
);

// Consensus State Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// Table to store client states.
    (WriteBatchSchema) u64 => WriteBatch
);
