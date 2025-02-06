use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    buf::Buf32,
    l1::{BitcoinAddress, Outpoint},
    params::{DepositTxParams, RollupParams},
    sorted_vec::SortedVec,
};

use crate::utils::{generate_taproot_address, get_operator_wallet_pks};

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct EnvelopeTags {
    pub checkpoint_tag: String,
    pub da_tag: String,
}

/// A configuration that determines how relevant transactions in a bitcoin block are filtered.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct TxFilterConfig {
    /// Envelope tag names
    pub envelope_tags: EnvelopeTags,

    /// For addresses that are expected to be spent to.
    pub expected_addrs: SortedVec<BitcoinAddress>,

    /// For blobs that are expected to be written to bitcoin.
    pub expected_blobs: SortedVec<Buf32>,

    /// For deposits that might be spent from.
    pub expected_outpoints: SortedVec<Outpoint>,

    /// Deposit config that determines how a deposit transaction can be parsed.
    pub deposit_config: DepositTxParams,
}

impl TxFilterConfig {
    /// Derive a `TxFilterConfig` from `RollupParams`.
    // TODO: this will need chainstate too in the future
    pub fn derive_from(rollup_params: &RollupParams) -> anyhow::Result<Self> {
        let operator_wallet_pks = get_operator_wallet_pks(rollup_params);
        let address = generate_taproot_address(&operator_wallet_pks, rollup_params.network)?;

        let rollup_name = rollup_params.rollup_name.clone();
        let expected_blobs = SortedVec::new(); // TODO: this should come from chainstate
        let expected_addrs = SortedVec::from(vec![address.clone()]);
        let expected_outpoints = SortedVec::new();

        let envelope_tags = EnvelopeTags {
            checkpoint_tag: rollup_params.checkpoint_tag.clone(),
            da_tag: rollup_params.da_tag.clone(),
        };

        let deposit_config = DepositTxParams {
            magic_bytes: rollup_name.clone().into_bytes(),
            address_length: rollup_params.address_length,
            deposit_amount: rollup_params.deposit_amount,
            address,
        };
        Ok(Self {
            envelope_tags,
            expected_blobs,
            expected_addrs,
            expected_outpoints,
            deposit_config,
        })
    }
}
