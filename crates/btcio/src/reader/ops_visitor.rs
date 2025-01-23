use strata_l1tx::filter::visitor::OpsVisitor;
use strata_primitives::buf::Buf32;
use strata_state::tx::ProtocolOperation;

/// Ops visitor for rollup client.
// TODO: add db manager
pub struct ClientOpsVisitor;

impl OpsVisitor for ClientOpsVisitor {
    fn visit_da(&self, _d: &[u8]) -> ProtocolOperation {
        // TODO: insert in db, when we have DA
        ProtocolOperation::DaCommitment(Buf32::zero())
    }
}
