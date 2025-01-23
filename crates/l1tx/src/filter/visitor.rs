use strata_primitives::hash;
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, DepositRequestInfo, ProtocolOperation},
};

pub trait OpsVisitor {
    // Do stuffs with `SignedBatchCheckpoint`.
    fn visit_checkpoint(&self, chkpt: SignedBatchCheckpoint) -> ProtocolOperation {
        ProtocolOperation::Checkpoint(chkpt)
    }

    // Do stuffs with `DepositInfo`.
    fn visit_deposit(&self, d: DepositInfo) -> ProtocolOperation {
        ProtocolOperation::Deposit(d)
    }

    // Do stuffs with `DepositRequest`.
    fn visit_deposit_request(&self, d: DepositRequestInfo) -> ProtocolOperation {
        ProtocolOperation::DepositRequest(d)
    }

    // Do stuffs with DA.
    fn visit_da(&self, d: &[u8]) -> ProtocolOperation {
        // TODO: this default implementation might change when we actually have DA
        let commitment = hash::raw(d);
        ProtocolOperation::DaCommitment(commitment)
    }
}
