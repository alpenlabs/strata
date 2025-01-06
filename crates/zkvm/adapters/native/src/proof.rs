use strata_zkvm::{ProofReceipt, ZkVmProofError};

#[derive(Debug, Clone)]
pub struct NativeProofReceipt(ProofReceipt);

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
