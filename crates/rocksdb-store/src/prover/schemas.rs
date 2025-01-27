use strata_primitives::proof::{ProofContext, ProofKey};
use zkaleido::ProofReceipt;

use crate::{define_table_with_default_codec, define_table_without_codec, impl_borsh_value_codec};

define_table_with_default_codec!(
    /// A table to store ProofKey -> ProofReceipt mapping
    (ProofSchema) ProofKey => ProofReceipt
);

define_table_with_default_codec!(
    /// A table to store dependencies of a proof context
    (ProofDepsSchema) ProofContext => Vec<ProofContext>
);
