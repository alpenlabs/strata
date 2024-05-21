use alpen_vertex_mmr::CompactMmr;
use alpen_vertex_primitives::{buf::Buf32, l1::L1Tx};

use crate::define_table_with_default_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;
use crate::traits::L1BlockManifest;

// Readability for header hash
type HeaderHash = Buf32;

// L1 Block Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store L1 Block data. Maps block index to header
    (L1BlockSchema) u64 => L1BlockManifest
);

// L1 Txns Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store L1 Txn data, maps block header hash to txns
    (TxnSchema) HeaderHash => Vec<L1Tx>
);

// Mmr Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store L1 Headers mmr
    (MmrSchema) u64 => CompactMmr // TODO: Properly define what the key should be
);
