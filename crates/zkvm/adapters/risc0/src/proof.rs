use risc0_zkvm::{InnerReceipt, Receipt};
use strata_zkvm::{Proof, ProofReceipt, PublicValues};

#[derive(Debug, Clone)]
pub struct Risc0ProofReceipt(Receipt);

impl Risc0ProofReceipt {
    pub fn inner(self) -> Receipt {
        self.0
    }
}

impl From<Receipt> for Risc0ProofReceipt {
    fn from(receipt: Receipt) -> Self {
        Risc0ProofReceipt(receipt)
    }
}

impl AsRef<Receipt> for Risc0ProofReceipt {
    fn as_ref(&self) -> &Receipt {
        &self.0
    }
}

impl From<ProofReceipt> for Risc0ProofReceipt {
    fn from(value: ProofReceipt) -> Self {
        Risc0ProofReceipt::from(&value)
    }
}

impl From<&ProofReceipt> for Risc0ProofReceipt {
    fn from(value: &ProofReceipt) -> Self {
        let journal = value.public_values().as_bytes().to_vec();
        let inner: InnerReceipt = bincode::deserialize(value.proof().as_bytes())
            .expect("bincode deserialization of Risc0 InnerReceipt failed");
        Receipt::new(inner, journal).into()
    }
}
impl From<Risc0ProofReceipt> for ProofReceipt {
    fn from(value: Risc0ProofReceipt) -> Self {
        let proof = Proof::new(
            bincode::serialize(&value.0.inner)
                .expect("bincode serialization for Risc0 InnerReceipt failed"),
        );
        let public_values = PublicValues::new(value.0.journal.bytes.to_vec());
        ProofReceipt::new(proof, public_values)
    }
}
