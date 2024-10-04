use strata_db::entities::bridge_tx_state::BridgeTxState;
use strata_primitives::buf::Buf32;
use strata_state::bridge_duties::BridgeDutyStatus;

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

define_table_with_seek_key_codec!(
    /// A table to store mapping of rocksdb index to [`Buf32`].
    (BridgeTxStateTxidSchema) u64 => Buf32
);

define_table_with_default_codec!(
    /// A table to map `Buf32` IDs to [`BridgeTxState`].
    (BridgeTxStateSchema) Buf32 => BridgeTxState
);

define_table_with_seek_key_codec!(
    /// A table to store mapping of [`Txid`] to [`Buf32`].
    (BridgeDutyTxidSchema) u64 => Buf32
);

define_table_with_default_codec!(
    /// A table to map `Buf32` IDs to [`BridgeDutyStatus`].
    (BridgeDutyStatusSchema) Buf32 => BridgeDutyStatus
);

define_table_with_default_codec!(
    /// A table to map rocksdb indexes to checkpoints.
    (BridgeDutyCheckpointSchema) u64 => u64
);
