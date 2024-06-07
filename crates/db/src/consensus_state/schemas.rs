use alpen_vertex_state::consensus::ConsensusState;
use alpen_vertex_state::operation::ConsensusOutput;

use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;

// Consensus Output Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store Consesus Output
    (ConsensusOutputSchema) u64 => ConsensusOutput
);

// Consensus State Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store Consesus State
    (ConsensusStateSchema) u64 => ConsensusState
);
