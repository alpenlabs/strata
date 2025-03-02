use bitcoin::Amount;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    block_credential::CredRule,
    buf::Buf32,
    l1::{BitcoinAddress, OutputRef},
    params::{DepositTxParams, RollupParams},
    sorted_vec::SortedVec,
};
use strata_state::{
    bridge_state::{DepositEntry, DepositState},
    chain_state::Chainstate,
};
use tracing::warn;

use crate::utils::{generate_taproot_address, get_operator_wallet_pks};

// TODO: This is FIXED OPERATOR FEE for TN1
pub const OPERATOR_FEE: Amount = Amount::from_int_btc(2);

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct EnvelopeTags {
    pub checkpoint_tag: String,
    pub da_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ExpectedWithdrawalFulfilment {
    /// withdrawal address
    pub destination: BitcoinAddress,
    /// Expected mininum withdrawal amount in sats
    pub amount: u64,
    /// index of assigned operator
    pub operator_idx: u32,
    /// index of assigned deposit entry for this withdrawal
    pub deposit_idx: u32,
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
    pub expected_outpoints: Vec<OutputRef>,

    /// For withdrawal fulfilment transactions sent by bridge operator.
    pub expected_withdrawal_fulfilments: Vec<ExpectedWithdrawalFulfilment>,

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
            expected_outpoints: Vec::new(),
            expected_withdrawal_fulfilments: Vec::new(),
            deposit_config,
        })
    }

    pub fn derive_from_chainstate(
        rollup_params: &RollupParams,
        chainstate: &Chainstate,
    ) -> anyhow::Result<Self> {
        let mut filterconfig = Self::derive_from(rollup_params)?;

        filterconfig.expected_withdrawal_fulfilments = derive_expected_withdrawal_fulfilments(
            chainstate.deposits_table().deposits(),
            rollup_params,
        );

        // Watch all utxos we have in our deposit table.
        filterconfig.expected_outpoints = chainstate
            .deposits_table()
            .deposits()
            .map(|deposit| *deposit.output())
            .collect();

        Ok(filterconfig)
    }
}

fn derive_expected_withdrawal_fulfilments<'a, I>(
    deposits: I,
    rollup_params: &RollupParams,
) -> Vec<ExpectedWithdrawalFulfilment>
where
    I: Iterator<Item = &'a DepositEntry>,
{
    deposits
        .filter_map(|deposit| match deposit.deposit_state() {
            // withdrawal has been assigned to an operator
            DepositState::Dispatched(dispatched_state) => {
                let expected =
                    dispatched_state
                        .cmd()
                        .withdraw_outputs()
                        .iter()
                        .filter_map(|output| {
                            let destination = match BitcoinAddress::from_descriptor(
                                output.destination(),
                                rollup_params.network,
                            ) {
                                Ok(address) => address,
                                Err(err) => {
                                    warn!(?output, ?err, "invalid withdrawal destination address");
                                    return None;
                                }
                            };
                            Some(ExpectedWithdrawalFulfilment {
                                destination,
                                // TODO: This uses FIXED OPERATOR FEE for TN1
                                amount: output.amt().to_sat().saturating_sub(OPERATOR_FEE.to_sat()),
                                operator_idx: dispatched_state.assignee(),
                                deposit_idx: deposit.idx(),
                            })
                        });
                Some(expected)
            }
            _ => None,
        })
        .flatten()
        .collect()
}
