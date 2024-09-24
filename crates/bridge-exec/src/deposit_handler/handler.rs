//! Defines the functions that pertain to handling a deposit.

use std::sync::Arc;

use alpen_express_btcio::rpc::traits::Signer;
use alpen_express_primitives::{
    l1::BitcoinTxid,
    relay::{types::Scope, util::MessageSigner},
};
use alpen_express_rpc_api::AlpenApiClient;
use bitcoin::{secp256k1::SECP256K1, Transaction, Txid};
use express_bridge_sig_manager::manager::SignatureManager;
use express_bridge_tx_builder::{deposit::DepositInfo, prelude::*, TxKind};
use jsonrpsee::tokio::time::{sleep, Duration};
use tracing::{debug, info};

use super::errors::DepositExecResult;
use crate::deposit_handler::errors::DepositExecError;

/// (Partially) signs the deposit transaction.
///
/// Also broadcasts to the bridge transaction database.
///
/// # Arguments
///
/// - `deposit_info`: a pending [`DepositInfo`] duty.
/// - `l1_rpc_client`: anything that is able to sign transactions and messages; i.e. holds private
///   keys.
/// - `l2_rpc_client`: anything that can communicate with the [`AlpenApiClient`].
/// - `sig_manager`: a stateful [`SignatureManager`].
/// - `tx_build_context`: stateful [`TxBuildContext`].
///
/// # Notes
///
/// Both the [`SignatureManager`] and the [`TxBuildContext`] can be reused
/// for multiple signing sessions if the operators'
/// [`PublickeyTable`](alpen_express_primitives::bridge::PublickeyTable)
/// does _not_ change.
///
/// We don't need mutexes since all functions to [`SignatureManager`] and
/// [`TxBuildContext`] takes non-mutable references.
pub async fn sign_deposit_tx(
    deposit_info: &DepositInfo,
    l1_rpc_client: &Arc<impl Signer>,
    l2_rpc_client: &Arc<impl AlpenApiClient + Sync>,
    sig_manager: &Arc<SignatureManager>,
    tx_build_context: &Arc<TxBuildContext>,
) -> DepositExecResult<Txid> {
    info!("starting deposit transaction signing");

    let operator_pubkeys = tx_build_context.pubkey_table();
    let own_index = tx_build_context.own_index();
    let own_pubkey = operator_pubkeys
        .0
        .get(&own_index)
        .expect("could not find operator's pubkey in public key table");

    info!(
        ?deposit_info,
        %own_index,
        %own_pubkey,
        "got the basic self information",
    );

    // sign the transaction with MuSig2 and put inside the OperatorPartialSig
    let xpriv = l1_rpc_client.get_xpriv().await?;
    if let Some(xpriv) = xpriv {
        let keypair = xpriv.to_keypair(SECP256K1);

        // construct the transaction data
        let tx_signing_data = deposit_info.construct_signing_data(tx_build_context.as_ref())?;

        debug!(?tx_signing_data, "got the signing data");

        // add the tx_state to the sig_manager in order to generate a sec_nonce and pub_nonce
        let txid = sig_manager
            .add_tx_state(tx_signing_data, operator_pubkeys.clone())
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;

        info!(
            %txid,
           "added the public nonce to the bridge transaction database",
        );

        // Then, submit_message RPC call
        let bitcoin_txid: BitcoinTxid = txid.into();

        let public_nonce = sig_manager
            .get_own_nonce(&txid)
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;

        let scope = Scope::V0DepositPubNonce(bitcoin_txid);
        debug!(?scope, "create the deposit pub nonce scope");
        let message = MessageSigner::new(own_index, keypair.secret_key().into())
            .sign_scope(&scope, &public_nonce)
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;
        debug!(?message, "create the deposit pub nonce message");
        let raw_message: Vec<u8> = message
            .try_into()
            .expect("could not serialize bridge message into raw bytes");

        l2_rpc_client.submit_bridge_msg(raw_message.into()).await?;
        info!("broadcasted the deposit pub nonce message");

        // Wait for all the pub nonces to be broadcasted by other operators.
        // Collect all nonces.
        // Then signing will not fail.
        loop {
            debug!("trying to get all pub nonces from the bridge transaction database, waiting for other operators' nonces");
            let got_all_nonces = sig_manager
                .get_tx_state(&txid)
                .await
                .map_err(|e| DepositExecError::TxState(e.to_string()))?
                .has_all_nonces();
            if got_all_nonces {
                info!(
                    %got_all_nonces, "got all pub nonces from the bridge transaction database",
                );
                break;
            } else {
                // TODO: this is hardcoded, maybe move this to a user-configurable Config
                sleep(Duration::from_millis(100)).await;
                continue;
            }
        }

        // adds the operator's partial signature
        // NOTE: this should be not fail now since we have all the pub nonces
        let flag = sig_manager
            .add_own_partial_sig(&txid)
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;

        info!(%txid, "added own operator's partial signature");

        // if the flag is true, then the PSBT is fully signed by all required operators
        if flag {
            info!(%txid, "deposit transaction fully signed");
        }

        Ok(txid)
    } else {
        Err(DepositExecError::Xpriv)
    }
}

/// Pools and aggregates signatures for the deposit transaction
/// into a fully-signed ready-to-be-broadcasted Bitcoin [`Transaction`].
///
/// Also broadcasts to the bridge transaction database.
///
/// # Arguments
///
/// - `txid`: a [`Txid`] that is in the bridge transaction database.
/// - `l1_rpc_client`: anything that is able to sign transactions and messages; i.e. holds private
///   keys.
/// - `l2_rpc_client`: anything that can communicate with the [`AlpenApiClient`].
/// - `sig_manager`: a stateful [`SignatureManager`].
/// - `tx_build_context`: stateful [`TxBuildContext`].
///
/// # Notes
///
/// Both the [`SignatureManager`] and the [`TxBuildContext`] can be reused
/// for multiple signing sessions if the operators'
/// [`PublickeyTable`](alpen_express_primitives::bridge::PublickeyTable)
/// does _not_ change.
///
/// We don't need mutexes since all functions to [`SignatureManager`] and
/// [`TxBuildContext`] takes non-mutable references.
pub async fn aggregate_deposit_sig(
    txid: &Txid,
    l1_rpc_client: &Arc<impl Signer>,
    l2_rpc_client: &Arc<impl AlpenApiClient + Sync>,
    sig_manager: &Arc<SignatureManager>,
    tx_build_context: &Arc<TxBuildContext>,
) -> DepositExecResult<Transaction> {
    info!("starting deposit transaction signature aggregation");

    let operator_pubkeys = tx_build_context.pubkey_table();
    let own_index = tx_build_context.own_index();
    let own_pubkey = operator_pubkeys
        .0
        .get(&own_index)
        .expect("could not find operator's pubkey in public key table");

    info!(
        %txid,
        %own_index,
        %own_pubkey,
         "got the basic self information",
    );

    let tx_state = sig_manager
        .get_tx_state(txid)
        .await
        .map_err(|e| DepositExecError::TxState(e.to_string()))?;

    debug!(
        %txid,
        ?tx_state,
        "got transaction state from bridge database",
    );

    // Fully signed and in the database, nothing to do here...
    if tx_state.is_fully_signed() {
        info!(
            %txid,
            "deposit transaction fully signed and in the bridge database",
        );
        let tx = sig_manager
            .finalize_transaction(txid)
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;
        return Ok(tx);
    }

    // Not fully signed, then partially sign transaction, construct, and sign a message
    let xpriv = l1_rpc_client.get_xpriv().await?;
    if let Some(xpriv) = xpriv {
        let keypair = xpriv.to_keypair(SECP256K1);

        // First check if it needs this operator's partial signature
        let needs_our_sig = tx_state.collected_sigs().get(&own_index).is_none();
        if needs_our_sig {
            sig_manager
                .add_own_partial_sig(txid)
                .await
                .map_err(|e| DepositExecError::Signing(e.to_string()))?;
            info!(
                %txid,
                "added own's partial signature to the bridge transaction database",
            );
        }

        // Now, get this operator's partial sig
        let partial_sig = sig_manager
            .get_own_partial_sig(txid)
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?
            .expect("should've been signed");

        info!(
            ?partial_sig,
            "got own's partial signature from the bridge transaction database",
        );

        // submit_message RPC call
        let bitcoin_txid: BitcoinTxid = (*txid).into();
        let scope = Scope::V0DepositSig(bitcoin_txid);
        debug!(?scope, "create the deposit partial signature scope");
        let message = MessageSigner::new(own_index, keypair.secret_key().into())
            .sign_scope(&scope, &partial_sig)
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;
        debug!(?message, "create the deposit partial signature message",);
        let raw_message: Vec<u8> = message
            .try_into()
            .expect("could not serialize bridge message into raw bytes");

        l2_rpc_client.submit_bridge_msg(raw_message.into()).await?;
        info!("broadcasted the withdrawal partial signature message");

        // Wait for all the partial signatures to be broadcasted by other operators.
        // Collect all partial signature.
        loop {
            debug!(
                "trying to get all partial signatures from the bridge transaction database, waiting for other operators' signatures"
            );
            let got_all_sigs = sig_manager
                .get_tx_state(txid)
                .await
                .map_err(|e| DepositExecError::TxState(e.to_string()))?
                .is_fully_signed();
            if got_all_sigs {
                info!(
                    %got_all_sigs,
                   "got all partial signatures from the bridge transaction database",
                );
                break;
            } else {
                // TODO: this is hardcoded, maybe move this to a user-configurable Config
                sleep(Duration::from_millis(100)).await;
                continue;
            }
        }
        let tx = sig_manager
            .finalize_transaction(txid)
            .await
            .map_err(|e| DepositExecError::Signing(e.to_string()))?;
        info!(%txid, "done deposit transaction signature aggregation");
        Ok(tx)
    } else {
        Err(DepositExecError::Xpriv)
    }
}
