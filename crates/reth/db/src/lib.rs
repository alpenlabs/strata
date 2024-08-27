pub mod rocksdb;
pub use alpen_express_db::{errors, DbResult};
use reth_primitives::B256;
use zkvm_primitives::ZKVMInput;

pub trait WitnessStore {
    fn put_block_witness(&self, block_hash: B256, witness: &ZKVMInput) -> DbResult<()>;
    fn del_block_witness(&self, block_hash: B256) -> DbResult<()>;
}

pub trait WitnessProvider {
    fn get_block_witness(&self, block_hash: B256) -> DbResult<Option<ZKVMInput>>;
    fn get_block_witness_raw(&self, block_hash: B256) -> DbResult<Option<Vec<u8>>>;
}
