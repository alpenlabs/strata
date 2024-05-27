use crate::traits::ConsensusOutput;

use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;

// L1 Block Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store Consesus State
    (ConsensusStateSchema) u64 => ConsensusOutput
);