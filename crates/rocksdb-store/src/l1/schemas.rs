use strata_mmr::CompactMmr;
use strata_primitives::{buf::Buf32, l1::EpochedL1BlockManifest};
use strata_state::l1::L1Tx;

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

// Readability for header hash
type HeaderHash = Buf32;

// L1 Block Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// A table to store L1 Block data. Maps block index to header
    (L1BlockSchema) u64 => EpochedL1BlockManifest
);

// L1 Txns Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store L1 Txn data, maps block header hash to txns
    (TxnSchema) HeaderHash => Vec<L1Tx>
);

// Mmr Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// A table to store L1 Headers mmr
    (MmrSchema) u64 => CompactMmr
);
