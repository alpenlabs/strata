use strata_mmr::CompactMmr;
use strata_primitives::l1::{L1BlockId, L1BlockManifest, L1Tx};

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

// L1 Block Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// A table to store L1 Block data. Maps block index to header
    (L1BlockSchema) u64 => L1BlockManifest
);

// L1 Txns Schema and corresponding codecs implementation
define_table_with_default_codec!(
    /// A table to store L1 Txn data, maps block header hash to txns
    (TxnSchema) L1BlockId => Vec<L1Tx>
);

// Mmr Schema and corresponding codecs implementation
define_table_with_seek_key_codec!(
    /// A table to store L1 Headers mmr
    (MmrSchema) u64 => CompactMmr
);
