use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use super::{DaTx, DepositUpdateTx, L1BlockId, L1HeaderPayload, L1HeaderRecord};

/// Entry representing an L1 block that we've acknowledged.
///
/// It seems to be on the longest chain but might still reorg. We wait until the block
/// is buried enough before accepting the block and acting on the relevant txs in it.
///
/// Height is implicit by its position in the maturation queue.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1MaturationEntry {
    /// Header record that contains the important proof information.
    record: L1HeaderRecord,

    /// Txs related to deposits.
    ///
    /// MUST be sorted by [`DepositUpdateTx`] index within block.
    deposit_update_txs: Vec<DepositUpdateTx>,

    /// Txs representing L1 DA.
    ///
    /// MUST be sorted by [`DaTx`] index within block.
    da_txs: Vec<DaTx>,
}

impl L1MaturationEntry {
    pub fn new(
        record: L1HeaderRecord,
        deposit_update_txs: Vec<DepositUpdateTx>,
        da_txs: Vec<DaTx>,
    ) -> Self {
        Self {
            record,
            deposit_update_txs,
            da_txs,
        }
    }

    /// Computes the L1 blockid from the stored block.
    pub fn blkid(&self) -> &L1BlockId {
        self.record.blkid()
    }

    pub fn into_parts(self) -> (L1HeaderRecord, Vec<DepositUpdateTx>, Vec<DaTx>) {
        (self.record, self.deposit_update_txs, self.da_txs)
    }
}

impl From<L1HeaderPayload> for L1MaturationEntry {
    fn from(value: L1HeaderPayload) -> Self {
        Self {
            record: value.record,
            deposit_update_txs: value.deposit_update_txs,
            da_txs: value.da_txs,
        }
    }
}
