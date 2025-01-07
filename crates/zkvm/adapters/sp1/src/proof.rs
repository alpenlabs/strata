use sp1_sdk::{SP1Proof, SP1ProofWithPublicValues, SP1PublicValues };
use strata_zkvm::{Proof, ProofReceipt, PublicValues, ZkVmProofError};

#[derive(Debug, Clone)]
pub struct SP1ProofReceipt(SP1ProofWithPublicValues);

impl SP1ProofReceipt {
    pub fn inner(self) -> SP1ProofWithPublicValues {
        self.0
    }
}

impl From<SP1ProofWithPublicValues> for SP1ProofReceipt {
    fn from(receipt: SP1ProofWithPublicValues) -> Self {
        SP1ProofReceipt(receipt)
    }
}

impl AsRef<SP1ProofWithPublicValues> for SP1ProofReceipt {
    fn as_ref(&self) -> &SP1ProofWithPublicValues {
        &self.0
    }
}

impl TryFrom<ProofReceipt> for SP1ProofReceipt {
    type Error = ZkVmProofError;
    fn try_from(value: ProofReceipt) -> Result<Self, Self::Error> {
        SP1ProofReceipt::try_from(&value)
    }
}

impl TryFrom<&ProofReceipt> for SP1ProofReceipt {
    type Error = ZkVmProofError;
    fn try_from(value: &ProofReceipt) -> Result<Self, Self::Error> {
        let public_values = SP1PublicValues::from(value.public_values().as_bytes());
        let proof: SP1Proof = bincode::deserialize(value.proof().as_bytes())
            .map_err(|e| ZkVmProofError::DataFormat(e.into()))?;
        let sp1_version = sp1_sdk::SP1_CIRCUIT_VERSION.to_string();
        let proof_receipt = SP1ProofWithPublicValues {
            proof,
            public_values,
            sp1_version,
        };
        Ok(SP1ProofReceipt(proof_receipt))
    }
}

impl TryFrom<SP1ProofReceipt> for ProofReceipt {
    type Error = ZkVmProofError;
    fn try_from(value: SP1ProofReceipt) -> Result<Self, Self::Error> {
        let proof = Proof::new(
            bincode::serialize(&value.0.proof).map_err(|e| ZkVmProofError::DataFormat(e.into()))?,
        );
        let public_values = PublicValues::new(value.0.public_values.to_vec());
        Ok(ProofReceipt::new(proof, public_values))
    }
}
