#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use alloy_primitives::U256;
use revm::db::{BundleAccount, BundleState};
use revm_primitives::{AccountInfo, Address, Bytecode, HashMap, B256};
use serde::{Deserialize, Serialize};

/// Represents full state diff for the block together with original values.
#[derive(Clone, Default, Debug, Deserialize, Serialize, PartialEq)]
pub struct BlockStateDiff {
    /// Account state.
    pub state: HashMap<Address, BundleAccount>,
    /// All created contracts in this block.
    pub contracts: HashMap<B256, Bytecode>,
}

impl From<BundleState> for BlockStateDiff {
    fn from(value: BundleState) -> Self {
        Self {
            state: value.state,
            contracts: value.contracts,
        }
    }
}

/// A representation of several [`BlockStateDiff`] ready to be serialized and posted on DA.
///
/// N.B. "original" counterparts are needed to correctly construct the [`BatchStateDiff`]
/// for the range of blocks and not include changes for the keys whose values
/// were changed inside batch, but end up having the same value as before the batch.
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct BatchStateDiff {
    /// An account representation in the diff.
    /// [`Option::None`] indicates the account was destructed, but existed before the batch.
    pub accounts: HashMap<Address, Option<AccountInfo>>,

    /// The *FIRST* original [`AccountInfo`] for the account in the batch.
    /// [`Option::None`] if the account was absent before the batch.
    #[serde(skip)]
    pub original_accounts: HashMap<Address, Option<AccountInfo>>,

    /// A collection of deployed smart contracts within the batch.
    pub contracts: HashMap<B256, Bytecode>,

    /// Storage slots.
    pub storage_slots: HashMap<Address, HashMap<U256, U256>>,
    /// The *FIRST* original value for the slot in the batch.
    #[serde(skip)]
    pub original_storage_slots: HashMap<Address, HashMap<U256, U256>>,
}

impl BatchStateDiff {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn apply(&mut self, diff: BlockStateDiff) {
        // Accumulate deployed contracts.
        for (addr, bytecode) in diff.contracts.into_iter() {
            self.contracts.insert(addr, bytecode);
        }

        for (addr, acc) in &diff.state {
            // Update original account if seen for the first time in the batch.
            if !self.original_accounts.contains_key(addr) {
                self.original_accounts
                    .insert(*addr, acc.original_info.clone());
            }

            // Now, modify the actual account entry.
            let cur_account = acc.account_info();
            if &cur_account == self.original_accounts.get(addr).unwrap() {
                // Remove if the actual account equals to the original.
                self.accounts.remove(addr);
            } else {
                // The current account is different, update it.
                self.accounts.insert(*addr, cur_account);
            }

            let original_acc_storage = self.original_storage_slots.entry(*addr).or_default();

            let cur_acc_storage = self.storage_slots.entry(*addr).or_default();

            for (key, value) in &acc.storage {
                // Update original account storage if seen for the first time in the batch.
                if !original_acc_storage.contains_key(key) {
                    original_acc_storage.insert(*key, value.original_value());
                }

                // Now, modify the actual account storage slots.
                let cur_storage_value = value.present_value();
                if &cur_storage_value == original_acc_storage.get(key).unwrap() {
                    // Remove if the actual value equals to the original.
                    cur_acc_storage.remove(key);
                } else {
                    // The current value is different, update it.
                    cur_acc_storage.insert(*key, cur_storage_value);
                }
            }
        }
    }
}
