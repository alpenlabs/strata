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

/// Interface to extract relevant information from a transaction.
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
    fn finalize(self) -> L1TxExtract;
}

/// Interface to extract relevant information from a block.
pub trait BlockIndexer {
    fn index_tx(&mut self, txidx: u32, tx: &Transaction);

    fn index_block(mut self, block: &Block) -> Self
    where
        Self: Sized,
    {
        for (i, tx) in block.txdata.iter().enumerate() {
            self.index_tx(i as u32, tx);
        }
        self
    }

    // Collect data
    fn finalize(self) -> L1BlockExtract;
}

/// Indexes `ProtocolTxEntry`s, `DepositRequestInfo`s and `DaEntry`s from a bitcoin block.
/// Currently, this is used from two contexts: rollup node and prover node, each of which will have
/// different `TxIndexer`s which determine what and how something is extracted from a transaction.
#[derive(Clone, Debug)]
pub struct OpIndexer<'a, T: TxIndexer> {
    /// The actual logic of what and how something is extracted from a transaction.
    tx_indexer: T,
    /// The config that's used to filter transactions and extract data. This has a lifetime
    /// parameter for two reasons: 1) It is used in prover context so using Arc might incur some
    /// overheads, 2) We can be sure that the config won't change during indexing of a l1 block.
    filter_config: &'a TxFilterConfig,
    /// `ProtocolTxEntry`s will be accumulated here.
    tx_entries: Vec<ProtocolTxEntry>,
    /// `DepositRequestInfo`s will be accumulated here.
    dep_reqs: Vec<DepositRequestInfo>,
    /// `DaEntry`s will be accumulated here.
    da_entries: Vec<DaEntry>,
}

impl<'a, T: TxIndexer> OpIndexer<'a, T> {
    pub fn new(tx_indexer: T, filter_config: &'a TxFilterConfig) -> Self {
        Self {
            tx_indexer,
            filter_config,
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

impl<V: TxIndexer + Clone> BlockIndexer for OpIndexer<'_, V> {
    fn index_tx(&mut self, txidx: u32, tx: &Transaction) {
        let mut tx_indexer = self.tx_indexer.clone();
        for chp in parse_checkpoint_envelopes(tx, self.filter_config) {
            tx_indexer.visit_checkpoint(chp);
        }

        for dp in parse_deposits(tx, self.filter_config) {
            tx_indexer.visit_deposit(dp);
        }

        // TODO: remove this later when we do not require deposit request ops
        for dp in parse_deposit_requests(tx, self.filter_config) {
            tx_indexer.visit_deposit_request(dp);
        }

        for da in parse_da_blobs(tx, self.filter_config) {
            tx_indexer.visit_da(da);
        }

        let tx_extract = tx_indexer.finalize();
        let (ops, mut deps, mut das) = tx_extract.into_parts();
        if !ops.is_empty() {
            let entry = ProtocolTxEntry::new(txidx, ops);
            self.tx_entries.push(entry);
        }

        self.dep_reqs.append(&mut deps);
        self.da_entries.append(&mut das);
    }

    fn finalize(self) -> L1BlockExtract {
        L1BlockExtract::new(self.tx_entries, self.dep_reqs, self.da_entries)
    }
}
