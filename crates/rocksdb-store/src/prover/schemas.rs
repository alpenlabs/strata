use strata_primitives::vk::StrataProofId;
use strata_zkvm::ProofWithInfo;
use uuid::Uuid;

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

// todo: use proper types after they are defined on state crate
define_table_with_seek_key_codec!(
    /// A table to store idx-> task id mapping
    (ProverTaskIdSchema) Uuid => StrataProofId
);

define_table_with_default_codec!(
    /// A table to store task id-> proof bytes mapping
    (ProverTaskSchema) Uuid => ProofWithInfo
);
