use risc0_zkvm::guest::env;

use revm::InMemoryDB;
use zkvm_primitives::db::InMemoryDBHelper;
use zkvm_primitives::processor::EvmProcessor;
use zkvm_primitives::ZKVMInput;

const ENCODED_IP: &[u8] = include_bytes!("../1.bin");

fn main() {
    // TODO: Read the input from the host
    let mut input: ZKVMInput = bincode::deserialize(ENCODED_IP).unwrap();

    // Initialize the database.
    let db = InMemoryDB::initialize(&mut input).unwrap();

    // Execute the block.
    let mut executor = EvmProcessor::<InMemoryDB> {
        input,
        db: Some(db),
        header: None,
    };

    executor.initialize();
    executor.execute();
    executor.finalize();

    // extract the public output
    let res = executor.header.unwrap().state_root;

    env::commit(&res);
}
