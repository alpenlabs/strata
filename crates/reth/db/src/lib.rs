pub mod errors;
pub mod rocksdb;

use reth_primitives::B256;
use zkvm_primitives::ZKVMInput;

use crate::errors::DbError;

pub type DbResult<T> = anyhow::Result<T, DbError>;

pub trait WitnessStore {
    fn put_block_witness(&self, block_hash: B256, witness: &ZKVMInput) -> DbResult<()>;
    fn del_block_witness(&self, block_hash: B256) -> DbResult<()>;
}

pub trait WitnessProvider {
    fn get_block_witness(&self, block_hash: B256) -> DbResult<Option<ZKVMInput>>;
    fn get_block_witness_raw(&self, block_hash: B256) -> DbResult<Option<Vec<u8>>>;
}
