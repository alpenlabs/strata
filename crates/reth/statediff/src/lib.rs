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

            // It can happen that the storage slots for a certain account has been completely
            // reverted, so remove the account from the map.
            if cur_acc_storage.is_empty() {
                self.storage_slots.remove(addr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use revm::db::{states::StorageSlot, BundleAccount};
    use revm_primitives::{
        alloy_primitives::map::HashMap, map::DefaultHashBuilder, AccountInfo, Address, FixedBytes,
        KECCAK_EMPTY, U256,
    };

    use crate::{BatchStateDiff, BlockStateDiff};

    const fn account1() -> Address {
        Address::new([0x60; 20])
    }

    const fn account2() -> Address {
        Address::new([0x61; 20])
    }

    fn acc_info1() -> AccountInfo {
        AccountInfo {
            nonce: 1,
            balance: U256::from(10),
            code_hash: KECCAK_EMPTY,
            code: None,
        }
    }

    fn acc_info2() -> AccountInfo {
        AccountInfo {
            nonce: 3,
            balance: U256::from(20),
            code_hash: KECCAK_EMPTY,
            code: None,
        }
    }

    fn slot1() -> U256 {
        U256::from(5)
    }

    fn slot2() -> U256 {
        U256::from(7)
    }

    fn slot_changes() -> HashMap<U256, StorageSlot> {
        HashMap::from_iter([
            (
                slot1(),
                StorageSlot::new_changed(U256::from(0), U256::from(10)),
            ),
            (
                slot2(),
                StorageSlot::new_changed(U256::from(10), U256::from(15)),
            ),
        ])
    }

    fn revert_slot_changes(
        slot_changes: &HashMap<U256, StorageSlot>,
    ) -> HashMap<U256, StorageSlot> {
        HashMap::<U256, StorageSlot, DefaultHashBuilder>::from_iter(slot_changes.iter().map(
            |(k, v)| {
                (
                    *k,
                    StorageSlot::new_changed(v.present_value, v.previous_or_original_value),
                )
            },
        ))
    }

    fn test_state_diff_acc1() -> BlockStateDiff {
        let mut test_diff = BlockStateDiff {
            state: HashMap::default(),
            contracts: HashMap::default(),
        };

        test_diff.state.insert(
            account1(),
            BundleAccount::new(
                None,
                Some(acc_info1()),
                slot_changes(),
                revm::db::AccountStatus::Changed,
            ),
        );

        test_diff
    }

    fn test_state_diff_acc_old_slots1() -> BlockStateDiff {
        let mut test_diff = BlockStateDiff {
            state: HashMap::default(),
            contracts: HashMap::default(),
        };

        test_diff.state.insert(
            account1(),
            BundleAccount::new(
                Some(acc_info1()),
                Some(AccountInfo {
                    nonce: 2,
                    balance: U256::from(9),
                    code_hash: KECCAK_EMPTY,
                    code: None,
                }),
                revert_slot_changes(&slot_changes()),
                revm::db::AccountStatus::Changed,
            ),
        );

        test_diff
    }

    fn test_state_diff_acc2() -> BlockStateDiff {
        let mut test_diff = BlockStateDiff {
            state: HashMap::default(),
            contracts: HashMap::default(),
        };

        test_diff.state.insert(
            account2(),
            BundleAccount::new(
                None,
                Some(acc_info2()),
                HashMap::default(),
                revm::db::AccountStatus::Changed,
            ),
        );

        test_diff
    }

    #[test]
    fn basic_batch_state_diff() {
        let mut batch_diff = BatchStateDiff::new();
        batch_diff.apply(test_state_diff_acc1());
        batch_diff.apply(test_state_diff_acc2());

        let acc1 = batch_diff
            .accounts
            .get(&account1())
            .expect("account1 should be present")
            .clone();
        let info1 = acc1.unwrap();
        assert!(info1 == acc_info1());

        let acc2 = batch_diff
            .accounts
            .get(&account2())
            .expect("account2 should be present")
            .clone();
        let info2 = acc2.unwrap();
        assert!(info2 == acc_info2());

        assert!(batch_diff.storage_slots.contains_key(&account1()));
    }

    #[test]
    fn multiple_slot_writes() {
        let mut batch_diff = BatchStateDiff::new();
        batch_diff.apply(test_state_diff_acc1());
        batch_diff.apply(test_state_diff_acc_old_slots1());

        let acc1 = batch_diff
            .accounts
            .get(&account1())
            .expect("account1 should be present")
            .clone();
        let info1 = acc1.unwrap();
        let expected_info = AccountInfo {
            nonce: 2,
            balance: U256::from(9),
            code_hash: KECCAK_EMPTY,
            code: None,
        };
        assert!(info1 == expected_info);
        // Slots were reverted to initial values, so the map should be empty.
        assert!(batch_diff.storage_slots.is_empty());

        // Apply slots again and check it was recorded.
        batch_diff.apply(test_state_diff_acc1());
        assert!(
            batch_diff
                .storage_slots
                .get(&account1())
                .unwrap()
                .get(&slot1())
                .unwrap()
                == &slot_changes().get(&slot1()).unwrap().present_value()
        )
    }

    #[test]
    fn smart_contract_diff() {
        let mut test_diff = BlockStateDiff {
            state: HashMap::default(),
            contracts: HashMap::default(),
        };
        test_diff.contracts.insert(
            FixedBytes::default(),
            revm_primitives::Bytecode::LegacyRaw(b"123".into()),
        );

        let mut batch_diff = BatchStateDiff::new();
        batch_diff.apply(test_diff);
        assert!(!batch_diff.contracts.is_empty())
    }
}
