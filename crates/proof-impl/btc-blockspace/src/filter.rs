//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use alpen_express_primitives::buf::Buf32;
use bitcoin::{opcodes::all::OP_RETURN, Block, ScriptBuf, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::logic::ScanRuleConfig;

const SOME_ALP_MAGIC: [u8; 32] = [1; 32];

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct DepositRequestData {
    /// Address of the rollup where the `amount` is deposited
    pub dest_addr: Vec<u8>,
    pub amount: u64,
}

/// TODO: reuse from [alpen_express_state]
pub type ForcedInclusion = Vec<u8>;
#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct StateUpdate {
    pub l1_state_hash: Buf32,
    pub l2_state_hash: Buf32,
    pub acc_pow: f64,
    pub proof: Vec<u8>,
}

/// Note: This needs to be consistent with the logic in other places
/// This is used as the placeholder logic for now
/// TODO: Use the same logic
fn extract_deposit(
    tx: &Transaction,
    bridge_scriptbufs: &[ScriptBuf],
) -> Option<DepositRequestData> {
    // DepositRequestData Tx
    // ________________________________________
    //            | n-of-n
    // txin       | 0.5 BTC
    // can        | ___________________________
    // be         | OP_RETURN
    // anything   | OP_PUSHBYTES_32 <ALP_MAGIC>
    //            | OP_PUSHBYTES_20 <ETH_ADDR>

    if tx.output.len() != 2 {
        return None;
    }

    // TODO: this will take some hints to do this more efficiently
    if bridge_scriptbufs
        .binary_search(&tx.output[0].script_pubkey)
        .is_err()
    {
        return None;
    }

    // OP_RETURN + OP_PUSHBYTES_32 + ALP_MAGIC + OP_PUSHBYTES_20 + ETH_ADDR
    const SIZE: usize = 1 + (1 + 32) + (1 + 20);
    let bytes = tx.output[1].script_pubkey.to_bytes();

    if bytes.len() != SIZE {
        return None;
    }

    if bytes[0] != OP_RETURN.to_u8() {
        return None;
    }

    if bytes[2..34] != SOME_ALP_MAGIC {
        return None;
    }

    Some(DepositRequestData {
        dest_addr: bytes[35..55].to_vec(),
        amount: tx.output[0].value.to_sat(),
    })
}

/// Note: This needs to be consistent with the logic in other places
/// This is used as the placeholder logic for now
/// TODO: Use the same logic
fn extract_forced_inclusion(_tx: &Transaction) -> Option<ForcedInclusion> {
    None
}

/// Note: This needs to be consistent with the logic in other places
/// This is used as the placeholder logic for now
/// TODO: Use the same logic
fn extract_state_update(_tx: &Transaction) -> Option<StateUpdate> {
    None
}

pub fn extract_relevant_transactions(
    block: &Block,
    scan_rule: &ScanRuleConfig,
) -> (
    Vec<DepositRequestData>,
    Vec<ForcedInclusion>,
    Option<StateUpdate>,
) {
    let mut deposits = Vec::new();
    let mut forced_inclusions = Vec::new();
    let mut state_update = None;

    for tx in &block.txdata {
        if let Some(deposit) = extract_deposit(tx, &scan_rule.bridge_scriptbufs) {
            deposits.push(deposit)
        }

        if let Some(forced_inclusion) = extract_forced_inclusion(tx) {
            forced_inclusions.push(forced_inclusion);
        }

        state_update = extract_state_update(tx).or(state_update);
    }

    (deposits, forced_inclusions, state_update)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitcoin::{consensus::Decodable, Address, Transaction};

    use super::extract_deposit;

    const BRIDGE_TX: &str = "0200000001f7c1f557fcbc9bf77bc82788a703f7ace4cc32ddc7c3ead9cec64807ab0cc0340000000000fdffffff0280f0fa02000000002251204fa32c175c962f58b831b28b57be26357ffe5882605e3cb1ce9b9da5660cb7ae0000000000000000376a200101010101010101010101010101010101010101010101010101010101010101147a62dd4d4a3eb30c8d10cefbb5d15ae1d899b3c700000000";

    const BRIDGE_ADDR: &str = "bcrt1pf73jc96ujch43wp3k294003xx4llukyzvp0revwwnww62esvk7hqvarg98";
    const ETH_USER: &str = "7a62dd4d4a3eb30c8d10cefbb5d15ae1d899b3c7";

    #[test]
    fn test_deposit() {
        let raw_tx = hex::decode(BRIDGE_TX).unwrap();
        let tx: Transaction = Decodable::consensus_decode(&mut raw_tx.as_slice()).unwrap();

        let bridge_address = Address::from_str(BRIDGE_ADDR).unwrap().assume_checked();
        let deposit = extract_deposit(&tx, &[bridge_address.script_pubkey()]);

        assert!(deposit
            .is_some_and(|deposit| deposit.amount == 50_000_000
                && hex::encode(deposit.dest_addr) == ETH_USER));
    }
}
