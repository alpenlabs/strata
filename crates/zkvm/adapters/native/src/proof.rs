use strata_zkvm::{ProofReceipt, ZkVmProofError};

#[derive(Debug, Clone)]
pub struct NativeProofReceipt(ProofReceipt);

impl NativeProofReceipt {
    pub fn inner(self) -> ProofReceipt {
        self.0
    }
}

impl TryFrom<ProofReceipt> for NativeProofReceipt {
    type Error = ZkVmProofError;
    fn try_from(value: ProofReceipt) -> Result<Self, Self::Error> {
        Ok(NativeProofReceipt(value))
    }
}

impl TryFrom<NativeProofReceipt> for ProofReceipt {
    type Error = ZkVmProofError;
    fn try_from(value: NativeProofReceipt) -> Result<Self, Self::Error> {
        Ok(value.0)
    }
}
