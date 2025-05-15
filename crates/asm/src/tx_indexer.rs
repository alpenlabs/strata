use strata_l1tx::filter::indexer::TxVisitor;
use strata_primitives::{
    batch::SignedCheckpoint,
    l1::{DaCommitment, DepositInfo, ProtocolOperation},
};

/// Ops indexer for use with the prover.
///
/// This just extracts *only* the protocol operations, in particular avoiding
/// copying the DA payload again, since memory copies are more expensive in
/// proofs.
#[derive(Debug, Clone)]
pub(crate) struct ASMTxVisitorImpl {
    ops: Vec<ProtocolOperation>,
}

impl ASMTxVisitorImpl {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }
}

impl TxVisitor for ASMTxVisitorImpl {
    type Output = Vec<ProtocolOperation>;

    fn visit_da<'a>(&mut self, chunks: impl Iterator<Item = &'a [u8]>) {
        let commitment = DaCommitment::from_chunk_iter(chunks);
        self.ops.push(ProtocolOperation::DaCommitment(commitment));
    }

    fn visit_deposit(&mut self, di: DepositInfo) {
        self.ops.push(ProtocolOperation::Deposit(di));
    }

    fn visit_checkpoint(&mut self, ckpt: SignedCheckpoint) {
        self.ops.push(ProtocolOperation::Checkpoint(ckpt));
    }

    fn finalize(self) -> Option<Vec<ProtocolOperation>> {
        if self.ops.is_empty() {
            None
        } else {
            Some(self.ops)
        }
    }
}
