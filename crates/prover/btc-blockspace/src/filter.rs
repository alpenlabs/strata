//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use std::str::FromStr;

use bitcoin::{opcodes::all::OP_RETURN, Address, Block, Transaction};
use serde::{Deserialize, Serialize};

use crate::logic::ScanParams;

const SOME_ALP_MAGIC: [u8; 32] = [1; 32];

#[derive(Debug, Serialize, Deserialize)]
pub struct Deposit {
    pub to: [u8; 20],
    pub amount: u64,
}

pub type ForcedInclusion = Vec<u8>;
pub type StateUpdate = Vec<u8>;

/// Note: This needs to be consistent with the logic in other places
/// This is used as the placeholder logic for now
/// TODO: Use the same logic
fn extract_deposit(tx: &Transaction, bridge_address: &Address) -> Option<Deposit> {
    // Deposit Tx
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

    if !bridge_address.matches_script_pubkey(&tx.output[0].script_pubkey) {
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

    Some(Deposit {
        to: bytes[35..55].try_into().unwrap(),
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
fn extract_state_update(_tx: &Transaction) -> Option<ForcedInclusion> {
    None
}

pub fn extract_relevant_transactions(
    block: &Block,
    scan_params: &ScanParams,
) -> (Vec<Deposit>, Vec<ForcedInclusion>, Vec<StateUpdate>) {
    let mut deposits = Vec::new();
    let mut forced_inclusions = Vec::new();
    let mut state_updates = Vec::new();

    let bridge_address = Address::from_str(&scan_params.bridge_address)
        .unwrap()
        .assume_checked();

    for tx in &block.txdata {
        if let Some(deposit) = extract_deposit(tx, &bridge_address) {
            deposits.push(deposit)
        }

        if let Some(forced_inclusion) = extract_forced_inclusion(tx) {
            forced_inclusions.push(forced_inclusion);
        }

        if let Some(state_update) = extract_state_update(tx) {
            state_updates.push(state_update);
        }
    }

    (deposits, forced_inclusions, state_updates)
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
        let deposit = extract_deposit(&tx, &bridge_address);

        assert!(deposit.is_some_and(
            |deposit| deposit.amount == 50_000_000 && hex::encode(deposit.to) == ETH_USER
        ));
    }
}
