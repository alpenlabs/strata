use bitcoin::{Block, Transaction};
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, DepositRequestInfo},
};

use super::{
    parse_checkpoint_envelopes, parse_da_blobs, parse_deposit_requests, parse_deposits,
    TxFilterConfig,
};
use crate::messages::{DaEntry, L1BlockExtract, L1TxExtract, ProtocolTxEntry};

pub trait TxIndexer {
    // Do stuffs with `SignedBatchCheckpoint`.
    fn visit_checkpoint(&mut self, _chkpt: SignedBatchCheckpoint) {}

    // Do stuffs with `DepositInfo`.
    fn visit_deposit(&mut self, _d: DepositInfo) {}

    // Do stuffs with `DepositRequest`.
    fn visit_deposit_request(&mut self, _d: DepositRequestInfo) {}

    // Do stuffs with DA.
    fn visit_da<'a>(&mut self, _d: impl Iterator<Item = &'a [u8]>) {}

    // Collect data
    fn collect(self) -> L1TxExtract;
}

pub trait BlockIndexer {
    fn index_tx(&mut self, txidx: u32, tx: &Transaction, config: &TxFilterConfig);

    fn index_block(mut self, block: &Block, config: &TxFilterConfig) -> Self
    where
        Self: Sized,
    {
        for (i, tx) in block.txdata.iter().enumerate() {
            self.index_tx(i as u32, tx, config);
        }
        self
    }

    // Collect data
    fn collect(self) -> L1BlockExtract;
}

#[derive(Clone, Debug)]
pub struct OpIndexer<V: TxIndexer> {
    visitor: V,
    tx_entries: Vec<ProtocolTxEntry>,
    dep_reqs: Vec<DepositRequestInfo>,
    da_entries: Vec<DaEntry>,
}

impl<V: TxIndexer> OpIndexer<V> {
    pub fn new(visitor: V) -> Self {
        Self {
            visitor,
            tx_entries: Vec::new(),
            dep_reqs: Vec::new(),
            da_entries: Vec::new(),
        }
    }

    pub fn tx_entries(&self) -> &[ProtocolTxEntry] {
        &self.tx_entries
    }

    pub fn deposit_requests(&self) -> &[DepositRequestInfo] {
        &self.dep_reqs
    }

    pub fn da_entries(&self) -> &[DaEntry] {
        &self.da_entries
    }
}

impl<V: TxIndexer + Clone> BlockIndexer for OpIndexer<V> {
    fn index_tx(&mut self, txidx: u32, tx: &Transaction, config: &TxFilterConfig) {
        let mut visitor = self.visitor.clone();
        for chp in parse_checkpoint_envelopes(tx, config) {
            visitor.visit_checkpoint(chp);
        }

        for dp in parse_deposits(tx, config) {
            visitor.visit_deposit(dp);
        }

        // TODO: remove this later when we do not require deposit request ops
        for dp in parse_deposit_requests(tx, config) {
            visitor.visit_deposit_request(dp);
        }

        for da in parse_da_blobs(tx, config) {
            visitor.visit_da(da);
        }

        let tx_extract = visitor.collect();
        if !tx_extract.protocol_ops().is_empty() {
            let entry = ProtocolTxEntry::new(txidx, tx_extract.protocol_ops().to_vec());
            self.tx_entries.push(entry);
        }

        self.dep_reqs.extend_from_slice(tx_extract.deposit_reqs());
        self.da_entries.extend_from_slice(tx_extract.da_entries());
    }

    fn collect(self) -> L1BlockExtract {
        L1BlockExtract::new(self.tx_entries, self.dep_reqs, self.da_entries)
    }
}
