use alpen_express_state::{batch::SignedBatchCheckpoint, tx::ProtocolOperation};
use bitcoin::{Block, Transaction};
use borsh::{BorshDeserialize, BorshSerialize};

use super::messages::ProtocolOpTxRef;
use crate::{
    deposit::{
        deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info,
        DepositTxConfig,
    },
    inscription::parse_inscription_data,
};

/// kind of transactions can be relevant for rollup node to filter
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum TxFilterRule {
    /// Inscription transactions with given Rollup name. This will be parsed by
    /// InscriptionParser which dictates the structure of inscription.
    RollupInscription(RollupName),
    /// Deposit transaction
    Deposit(DepositTxConfig),
}

type RollupName = String;

/// Filter all the relevant [`Transaction`]s in a block based on given [`TxFilterRule`]s
pub fn filter_relevant_txs(block: &Block, filters: &[TxFilterRule]) -> Vec<ProtocolOpTxRef> {
    block
        .txdata
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            check_and_extract_relevant_info(tx, filters)
                .map(|relevant_tx| ProtocolOpTxRef::new(i as u32, relevant_tx))
        })
        .collect()
}

///  if a [`Transaction`] is relevant based on given [`RelevantTxType`]s then we extract relevant
///  info
fn check_and_extract_relevant_info(
    tx: &Transaction,
    filters: &[TxFilterRule],
) -> Option<ProtocolOperation> {
    filters.iter().find_map(|rel_type| match rel_type {
        TxFilterRule::RollupInscription(name) => {
            if !tx.input.is_empty() {
                if let Some(scr) = tx.input[0].witness.tapscript() {
                    if let Ok(inscription_data) = parse_inscription_data(&scr.into(), name) {
                        if let Ok(signed_batch) = borsh::from_slice::<SignedBatchCheckpoint>(
                            inscription_data.batch_data(),
                        ) {
                            return Some(ProtocolOperation::RollupInscription(signed_batch));
                        }
                    }
                }
            }
            None
        }

        TxFilterRule::Deposit(config) => {
            if let Some(deposit_info) = extract_deposit_info(tx, config) {
                return Some(ProtocolOperation::Deposit(deposit_info));
            }

            if let Some(deposit_req_info) = extract_deposit_request_info(tx, config) {
                return Some(ProtocolOperation::DepositRequest(deposit_req_info));
            }

            None
        }
    })
}
