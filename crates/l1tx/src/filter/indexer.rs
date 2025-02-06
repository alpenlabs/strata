use bitcoin::{Block, Transaction};
use strata_state::{
    batch::SignedBatchCheckpoint,
    tx::{DepositInfo, DepositRequestInfo},
};

use super::{
    parse_checkpoint_envelopes, parse_da_blobs, parse_deposit_requests, parse_deposits,
    TxFilterConfig,
};
use crate::messages::IndexedTxEntry;

/// Interface to handle storage of extracted information from a transaction.
pub trait TxVisitor {
    /// Output type collecting what we want to extract from a tx.
    type Output;

    /// Do stuffs with `SignedBatchCheckpoint`.
    fn visit_checkpoint(&mut self, _chkpt: SignedBatchCheckpoint) {}

    /// Do stuffs with `DepositInfo`.
    fn visit_deposit(&mut self, _d: DepositInfo) {}

    /// Do stuffs with `DepositRequest`.
    fn visit_deposit_request(&mut self, _d: DepositRequestInfo) {}

    /// Do stuffs with DA.
    fn visit_da<'a>(&mut self, _d: impl Iterator<Item = &'a [u8]>) {}

    /// Export the indexed data, if it rose to the level of being useful.
    fn finalize(self) -> Option<Self::Output>;
}

/// Extracts a list of interesting transactions from a block according to a
/// provided visitor, with parts extracted from a provided filter config.
pub fn index_block<V: TxVisitor>(
    block: &Block,
    visitor_fn: impl Fn() -> V,
    config: &TxFilterConfig,
) -> Vec<IndexedTxEntry<V::Output>> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            index_tx(tx, visitor_fn(), config).map(|outp| IndexedTxEntry::new(i as u32, outp))
        })
        .collect::<Vec<_>>()
}

fn index_tx<V: TxVisitor>(
    tx: &Transaction,
    mut visitor: V,
    filter_config: &TxFilterConfig,
) -> Option<V::Output> {
    for ckpt in parse_checkpoint_envelopes(tx, filter_config) {
        visitor.visit_checkpoint(ckpt);
    }

    for dp in parse_deposits(tx, filter_config) {
        visitor.visit_deposit(dp);
    }

    for da in parse_da_blobs(tx, filter_config) {
        visitor.visit_da(da);
    }

    // TODO: maybe remove this later when we do not require deposit request ops?
    for dr in parse_deposit_requests(tx, filter_config) {
        visitor.visit_deposit_request(dr);
    }

    visitor.finalize()
}

/*
/// Interface to extract relevant information from a block.
pub trait BlockIndexer {
    /// Output from the indexing pass.
    type Output;

    /// Indexes the block and produces the output.
    fn index_block(&self, block: &Block) -> Self::Output;
}

/// Indexes `ProtocolTxEntry`s, `DepositRequestInfo`s and `DaEntry`s from a bitcoin block.
///
/// Currently, this is used from two contexts: rollup node and prover node, each of which will have
/// different `TxIndexer`s which determine what and how something is extracted from a transaction.
pub struct TxOpIndexer<'a, T: TxVisitor, F: Fn() -> T> {
    /// The actual logic of what and how something is extracted from a transaction.
    tx_indexer_fn: F,

    /// The config that's used to filter transactions and extract data. This has a lifetime
    /// parameter for two reasons: 1) It is used in prover context so using Arc might incur some
    /// overheads, 2) We can be sure that the config won't change during indexing of a l1 block.
    filter_config: &'a TxFilterConfig,

    /// `ProtocolTxEntry`s will be accumulated here.
    relevant_txs: Vec<IndexedTxEntry<T::Output>>,
}

impl<'a, T: TxVisitor, F: Fn() -> T> TxOpIndexer<'a, T, F> {
    pub fn new(tx_indexer_fn: F, filter_config: &'a TxFilterConfig) -> Self {
        Self {
            tx_indexer_fn,
            filter_config,
            relevant_txs: Vec::new(),
            dep_reqs: Vec::new(),
            da_entries: Vec::new(),
        }
    }

    pub fn tx_entries(&self) -> &[IndexedTxEntry<T::Output>] {
        &self.relevant_txs
    }

    pub fn deposit_requests(&self) -> &[DepositRequestInfo] {
        &self.dep_reqs
    }

    pub fn da_entries(&self) -> &[DaEntry] {
        &self.da_entries
    }
}

impl<T: TxVisitor, F: Fn() -> T> TxOpIndexer<'_, T, F> {
    fn index_tx(&self, txidx: u32, tx: &Transaction) {
        let mut tx_visitor = (self.tx_indexer_fn)();

        for ckpt in parse_checkpoint_envelopes(tx, self.filter_config) {
            tx_visitor.visit_checkpoint(ckpt);
        }

        for dp in parse_deposits(tx, self.filter_config) {
            tx_visitor.visit_deposit(dp);
        }

        // TODO: remove this later when we do not require deposit request ops
        for dr in parse_deposit_requests(tx, self.filter_config) {
            tx_visitor.visit_deposit_request(dr);
        }

        for da in parse_da_blobs(tx, self.filter_config) {
            tx_visitor.visit_da(da);
        }

        // Finalize the visitor.  If there's nothing to report then return
        // immeditately.
        if let Some(summary) = tx_visitor.finalize() {
            self.relevant_txs.push(IndexedTxEntry::new(txidx, summary));
        }

        self.dep_reqs.append(&mut deps);
        self.da_entries.append(&mut das);
    }
}

impl<T: TxVisitor, F: Fn() -> T> BlockIndexer for TxOpIndexer<'_, T, F> {
    type Output = Vec<RelevantTxEntry>;

    fn index_block(&self, block: &Block) -> Self::Output
    where
        Self: Sized,
    {
        let mut op_txs = Vec::new();
        let mut deposit_reqs = Vec::new();
        let mut da_entries = Vec::new();

        for (i, tx) in block.txdata.iter().enumerate() {
            self.index_tx(i as u32, tx);
        }

        // TODO
    }

    /*fn finalize(self) -> L1BlockExtract {
        L1BlockExtract::new(self.tx_entries, self.dep_reqs, self.da_entries)
    }*/
}*/

/// Generic no-op tx indexer that emits nothing for every tx but could
/// substitute for any type of visitor.
pub struct NopTxVisitorImpl<T>(::std::marker::PhantomData<T>);

impl<T> TxVisitor for NopTxVisitorImpl<T> {
    type Output = T;

    fn finalize(self) -> Option<Self::Output> {
        None
    }
}
