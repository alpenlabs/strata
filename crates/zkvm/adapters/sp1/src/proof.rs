use sp1_sdk::{SP1Proof, SP1ProofWithPublicValues, SP1PublicValues, SP1Stdin};
use strata_zkvm::{Proof, ProofReceipt, PublicValues};

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

impl From<ProofReceipt> for SP1ProofReceipt {
    fn from(value: ProofReceipt) -> Self {
        SP1ProofReceipt::from(&value)
    }
}

impl From<&ProofReceipt> for SP1ProofReceipt {
    fn from(value: &ProofReceipt) -> Self {
        let public_values = SP1PublicValues::from(value.public_values().as_bytes());
        let proof: SP1Proof = bincode::deserialize(value.proof().as_bytes())
            .expect("bincode deserialization of SP1Proof failed");
        let sp1_version = sp1_sdk::SP1_CIRCUIT_VERSION.to_string();
        let proof_receipt = SP1ProofWithPublicValues {
            proof,
            public_values,
            stdin: SP1Stdin::default(),
            sp1_version,
        };
        SP1ProofReceipt(proof_receipt)
    }
}
impl From<SP1ProofReceipt> for ProofReceipt {
    fn from(value: SP1ProofReceipt) -> Self {
        let proof = Proof::new(
            bincode::serialize(&value.0.proof).expect("bincode serialization of SP1Proof failed"),
        );
        let public_values = PublicValues::new(value.0.public_values.to_vec());
        ProofReceipt::new(proof, public_values)
    }
}
