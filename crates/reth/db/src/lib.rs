pub mod rocksdb;
use reth_primitives::revm_primitives::alloy_primitives::B256;
pub use strata_db::{errors, DbResult};
use strata_proofimpl_evm_ee_stf::ELProofInput;

pub trait WitnessStore {
    fn put_block_witness(&self, block_hash: B256, witness: &ELProofInput) -> DbResult<()>;
    fn del_block_witness(&self, block_hash: B256) -> DbResult<()>;
}

pub trait WitnessProvider {
    fn get_block_witness(&self, block_hash: B256) -> DbResult<Option<ELProofInput>>;
    fn get_block_witness_raw(&self, block_hash: B256) -> DbResult<Option<Vec<u8>>>;
}
