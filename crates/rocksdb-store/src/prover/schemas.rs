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
    (ProverTaskIdSchema) u64 => [u8; 16]
);

define_table_with_default_codec!(
    /// A table to store task id-> proof bytes mapping
    (ProverTaskSchema) [u8; 16] => Vec<u8>
);

// todo: use proper types after they are defined on state crate
define_table_with_seek_key_codec!(
    /// A table to store task id -> StrataProofId
    (ProofIdSchema) Uuid => StrataProofId
);

define_table_with_default_codec!(
    /// A table to store task id > ProofWithInfo
    (ProverInfoSchema) Uuid => ProofWithInfo
);

define_table_with_default_codec!(
    /// A table to store task id and it's dependencies
    (ProofDependenciesSchema) Uuid => Vec<Uuid>
);
