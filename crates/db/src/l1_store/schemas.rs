use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::l1::{L1HeaderPayload, L1Tx};

use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;

define_table_with_default_codec!(
    /// A table to store L1 Block data
    (L1BlockSchema) Buf32 => L1HeaderPayload
);

define_table_with_default_codec!(
    /// A table to store L1 Txn data
    (TxnSchema) Buf32 => L1Tx
);

// Mmr Schema and corresponding codecs implementation
type MmrKey = u32; // TODO: change appropriately
type MmrValue = u32; // TODO: change appropriately

define_table_with_default_codec!(
    /// A table to store L1 Headers mmr
    (MmrSchema) MmrKey => MmrValue
);
