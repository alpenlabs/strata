use bitcoin::{Block, Transaction};
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, DepositRequestInfo, ProtocolOperation},
};

use super::{
    parse_checkpoint_envelopes, parse_da_blobs, parse_deposit_requests, parse_deposits,
    TxFilterConfig,
};
use crate::messages::ProtocolTxEntry;

pub trait OpsVisitor {
    // Do stuffs with `SignedBatchCheckpoint`.
    fn visit_checkpoint(&mut self, _chkpt: SignedBatchCheckpoint) {}

    // Do stuffs with `DepositInfo`.
    fn visit_deposit(&mut self, _d: DepositInfo) {}

    // Do stuffs with `DepositRequest`.
    fn visit_deposit_request(&mut self, _d: DepositRequestInfo) {}

    // Do stuffs with DA.
    fn visit_da<'a>(&mut self, _d: impl Iterator<Item = &'a [u8]>) {}

    fn collect(self) -> Vec<ProtocolOperation>;
}

pub trait BlockIndexer {
    type Output;

    fn collect(self) -> Self::Output;

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
}

#[derive(Clone, Debug)]
pub struct OpIndexer<V: OpsVisitor> {
    visitor: V,
    tx_entries: Vec<ProtocolTxEntry>,
}

impl<V: OpsVisitor> OpIndexer<V> {
    pub fn new(visitor: V) -> Self {
        Self {
            visitor,
            tx_entries: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct DepositRequestIndexer {
    requests: Vec<DepositRequestInfo>,
}

impl DepositRequestIndexer {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl<V: OpsVisitor + Clone> BlockIndexer for OpIndexer<V> {
    type Output = Vec<ProtocolTxEntry>;

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

        let ops = visitor.collect();
        if !ops.is_empty() {
            let entry = ProtocolTxEntry::new(txidx, ops);
            self.tx_entries.push(entry);
        }
    }

    fn collect(self) -> Self::Output {
        self.tx_entries
    }
}

impl BlockIndexer for DepositRequestIndexer {
    type Output = Vec<DepositRequestInfo>;

    fn collect(self) -> Self::Output {
        self.requests
    }

    fn index_tx(&mut self, _txidx: u32, tx: &Transaction, config: &TxFilterConfig) {
        self.requests.extend(parse_deposit_requests(tx, config));
    }
}
