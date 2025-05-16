pub mod diff;

use std::collections::HashMap;

use diff::{DaSerializable, HashMapDiff, NumDiff, RegisterDiff};

/// Dummy type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buf32(pub [u8; 32]);

/// Dummy type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64,
    pub code_hash: Buf32,
    pub storage_hash: Buf32,
}

/// Dummy state type
pub struct State {
    pub accounts: HashMap<Buf32, Account>,
    pub l1_hashes: Vec<Buf32>,
    pub curr_epoch: u64,
    pub curr_slot: u64,
    pub curr_root: Buf32,
    pub curr_parent: Buf32,
}

type StfInput = ();

struct StateDiff {
    pub accounts_diff: Vec<HashMapDiff<Buf32, Account>>,
    pub l1_hashes_diff: Vec<RegisterDiff<Buf32>>,
    pub curr_epoch_diff: Vec<NumDiff<u64, u8>>,
    pub curr_slot_diff: Vec<NumDiff<u64, u8>>,
    pub curr_root_diff: Vec<RegisterDiff<Buf32>>,
    pub curr_parent_diff: Vec<RegisterDiff<Buf32>>,
}

impl DaSerializable for StateDiff {
    fn serialize(&self) -> Vec<u8> {
        Vec::new()
    }

    fn deserialize(data: &[u8]) -> Self {
        todo!()
    }
}

fn state_transition(prev: State, input: StfInput) -> (State, StateDiff) {
    todo!()
}
