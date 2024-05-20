use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::l1::{L1HeaderPayload, L1Tx};

// TODO: this will later be concretely defined by @Trey
pub trait L1StoreTrait {
    /// Add a header to the store. Not necessarily to the persistent layer.
    fn put_header(&self, header_hash: Buf32, header_payload: L1HeaderPayload)
        -> anyhow::Result<()>;

    /// Add a transaction to the store
    fn put_transaction(&self, txid: Buf32, tx: L1Tx) -> anyhow::Result<()>;

    /// Write to the database. All the put_* operations will be in memory
    fn commit(&mut self) -> anyhow::Result<()>;

    /// Revert the store by nblocks. Usually happens when there is reorg in L1
    fn revert(&mut self, nblocks: u32) -> anyhow::Result<()>;

    /// Get header from store. Probably first look into cache
    fn get_header(&self, header_hash: Buf32) -> Option<L1HeaderPayload>;

    /// Get transaction from store. Probably first look into the cache
    fn get_transaction(&self, txid: Buf32) -> Option<L1Tx>;
}
