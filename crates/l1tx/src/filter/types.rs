use std::collections::HashMap;

use bitcoin::Amount;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    block_credential::CredRule,
    buf::Buf32,
    l1::{BitcoinAddress, BitcoinScriptBuf, OutputRef},
    params::{DepositTxParams, RollupParams},
    sorted_vec::{HasKey, SortedVec, SortedVecWithKey},
};
use strata_state::{
    bridge_state::{DepositEntry, DepositState},
    chain_state::Chainstate,
};

use crate::utils::{generate_taproot_address, get_operator_wallet_pks};

// TODO: This is FIXED OPERATOR FEE for TN1
pub const OPERATOR_FEE: Amount = Amount::from_int_btc(2);

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct EnvelopeTags {
    pub checkpoint_tag: String,
    pub da_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ExpectedWithdrawalFulfillment {
    /// withdrawal address scriptbuf
    pub destination: BitcoinScriptBuf,
    /// Expected minimum withdrawal amount in sats
    pub amount: u64,
    /// index of assigned operator
    pub operator_idx: u32,
    /// index of assigned deposit entry for this withdrawal
    pub deposit_idx: u32,
}

impl HasKey<BitcoinScriptBuf> for ExpectedWithdrawalFulfillment {
    fn get_key(&self) -> BitcoinScriptBuf {
        self.destination.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct DepositSpendConfig {
    /// index of deposit entry
    pub deposit_idx: u32,
    /// utxo for this deposit
    pub output: OutputRef,
}

impl HasKey<OutputRef> for DepositSpendConfig {
    fn get_key(&self) -> OutputRef {
        self.output
    }
}

/// A configuration that determines how relevant transactions in a bitcoin block are filtered.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct TxFilterConfig {
    /// Envelope tag names
    pub envelope_tags: EnvelopeTags,

    /// Rules for verifying sequencer signature
    pub sequencer_cred_rule: CredRule,

    /// For addresses that are expected to be spent to.
    pub expected_addrs: SortedVec<BitcoinAddress>,

    /// For blobs that are expected to be written to bitcoin.
    pub expected_blobs: SortedVec<Buf32>,

    /// For deposits that might be spent from.
    pub expected_outpoints: SortedVecWithKey<OutputRef, DepositSpendConfig>,

    /// For withdrawal fulfillment transactions sent by bridge operator. Maps deposit idx to
    /// fulfillment details.
    pub expected_withdrawal_fulfillments: HashMap<u32, ExpectedWithdrawalFulfillment>,

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
        let expected_addrs = SortedVec::from(vec![address.clone()]);
        let sequencer_cred_rule = rollup_params.cred_rule.clone();

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
            sequencer_cred_rule,
            expected_addrs,
            expected_blobs: SortedVec::new(),
            expected_outpoints: Vec::new().into(),
            expected_withdrawal_fulfillments: HashMap::new(),
            deposit_config,
        })
    }

    pub fn update_from_chainstate(&mut self, chainstate: &Chainstate) {
        self.expected_withdrawal_fulfillments =
            derive_expected_withdrawal_fulfillments(chainstate.deposits_table().deposits());

        // Watch all utxos we have in our deposit table.
        self.expected_outpoints = chainstate
            .deposits_table()
            .deposits()
            .map(|deposit| DepositSpendConfig {
                deposit_idx: deposit.idx(),
                output: *deposit.output(),
            })
            .collect::<Vec<_>>()
            .into();
    }
}

pub(crate) fn derive_expected_withdrawal_fulfillments<'a, I>(
    deposits: I,
) -> HashMap<u32, ExpectedWithdrawalFulfillment>
where
    I: Iterator<Item = &'a DepositEntry>,
{
    let fulfillments = deposits
        .filter_map(|deposit| match deposit.deposit_state() {
            // withdrawal has been assigned to an operator
            DepositState::Dispatched(dispatched_state) => {
                let expected = dispatched_state
                    .cmd()
                    .withdraw_outputs()
                    .iter()
                    .map(|output| {
                        (
                            deposit.idx(),
                            ExpectedWithdrawalFulfillment {
                                destination: output.destination().to_script().into(),
                                // TODO: This uses FIXED OPERATOR FEE for TN1
                                amount: output.amt().to_sat().saturating_sub(OPERATOR_FEE.to_sat()),
                                operator_idx: dispatched_state.assignee(),
                                deposit_idx: deposit.idx(),
                            },
                        )
                    });
                Some(expected)
            }
            _ => None,
        })
        .flatten();
    HashMap::from_iter(fulfillments)
}
