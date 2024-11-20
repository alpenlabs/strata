use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    buf::Buf32,
    l1::BitcoinAddress,
    params::{DepositTxParams, RollupParams},
};

use crate::utils::{generate_taproot_address, get_operator_wallet_pks};

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct TxFilterConfig {
    /// For checkpoint update inscriptions.
    pub rollup_name: String,

    /// For addresses that we expect spends to.
    // TODO: ensure sorted vec, possibly by having a separate SortedVec type
    pub expected_addrs: Vec<BitcoinAddress>,

    /// For blobs we expect to be written.
    pub expected_blobs: Vec<Buf32>,

    /// For deposits that might be spent from.
    pub expected_outpoints: Vec<Outpoint>,

    /// Deposit config that defines the structure we expect in the utxo
    pub deposit_config: DepositTxParams,
}

impl TxFilterConfig {
    // TODO: this will need chainstate too in the future
    pub fn derive_from(rollup_params: &RollupParams) -> anyhow::Result<Self> {
        let operator_wallet_pks = get_operator_wallet_pks(rollup_params);
        let address = generate_taproot_address(&operator_wallet_pks, rollup_params.network)?;

        let rollup_name = rollup_params.rollup_name.clone();
        let expected_blobs = Vec::new(); // TODO: this should come from chainstate
        let expected_addrs = vec![address.clone()];
        let expected_outpoints = Vec::new();

        let deposit_config = DepositTxParams {
            magic_bytes: rollup_name.clone().into_bytes(),
            address_length: rollup_params.address_length,
            deposit_amount: rollup_params.deposit_amount,
            address,
        };
        Ok(Self {
            rollup_name,
            expected_blobs,
            expected_addrs,
            expected_outpoints,
            deposit_config,
        })
    }
}

/// Outpoint of a bitcoin tx
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Outpoint {
    pub txid: Buf32,
    pub vout: u32,
}
