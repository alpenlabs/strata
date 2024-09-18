//! Defines the functions that pertain to handling a withdrawal request.

use std::sync::Arc;

use alpen_express_btcio::rpc::traits::Signer;
use alpen_express_primitives::{
    l1::BitcoinTxid,
    relay::{types::Scope, util::MessageSigner},
};
use alpen_express_rpc_api::AlpenApiClient;
use bitcoin::{secp256k1::SECP256K1, Txid};
use express_bridge_sig_manager::manager::SignatureManager;
use express_bridge_tx_builder::{prelude::*, withdrawal::CooperativeWithdrawalInfo, TxKind};
use jsonrpsee::tokio::time::{sleep, Duration};
use tracing::{debug, info};

use crate::withdrawal_handler::errors::{WithdrawalExecError, WithdrawalExecResult};

/// (Partially) signs the withdrawal transaction.
///
/// Also broadcasts to the bridge transaction database.
///
/// # Arguments
///
/// - `withdrawal_info`: a pending [`CooperativeWithdrawalInfo`] duty.
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
pub async fn sign_withdrawal_tx(
    withdrawal_info: &CooperativeWithdrawalInfo,
    l1_rpc_client: &Arc<impl Signer>,
    l2_rpc_client: &Arc<impl AlpenApiClient + Sync>,
    sig_manager: &Arc<SignatureManager>,
    tx_build_context: &Arc<TxBuildContext>,
) -> WithdrawalExecResult<Txid> {
    info!("starting withdrawal transaction signing");

    let operator_pubkeys = tx_build_context.pubkey_table();
    let own_index = tx_build_context.own_index();
    let own_pubkey = operator_pubkeys
        .0
        .get(&own_index)
        .expect("could not find operator's pubkey in public key table");

    info!(
        ?withdrawal_info,
        %own_index,
        %own_pubkey,
        "got the basic self information",
    );

    // sign the transaction with MuSig2 and put inside the OperatorPartialSig
    let xpriv = l1_rpc_client.get_xpriv().await?;
    if let Some(xpriv) = xpriv {
        let keypair = xpriv.to_keypair(SECP256K1);

        // construct the transaction data
        let tx_signing_data = withdrawal_info.construct_signing_data(tx_build_context.as_ref())?;

        debug!(?tx_signing_data, "got the signing data");

        // add the tx_state to the sig_manager in order to generate a sec_nonce and pub_nonce
        let txid = sig_manager
            .add_tx_state(tx_signing_data, operator_pubkeys.clone())
            .await
            .map_err(|e| WithdrawalExecError::Signing(e.to_string()))?;

        info!(
            %txid,
            "added the public nonce to the bridge transaction database",
        );

        // Then, submit_message RPC call
        let bitcoin_txid: BitcoinTxid = txid.into();

        let public_nonce = sig_manager
            .get_own_nonce(&txid)
            .await
            .map_err(|e| WithdrawalExecError::Signing(e.to_string()))?;

        let scope = Scope::V0WithdrawalPubNonce(bitcoin_txid);
        debug!(?scope, "create the withdrawal pub nonce scope");
        let message = MessageSigner::new(own_index, keypair.secret_key().into())
            .sign_scope(&scope, &public_nonce)
            .map_err(|e| WithdrawalExecError::Signing(e.to_string()))?;
        debug!(?message, "create the withdrawal pub nonce message");
        let raw_message: Vec<u8> = message
            .try_into()
            .expect("could not serialize bridge message into raw bytes");

        l2_rpc_client.submit_bridge_msg(raw_message.into()).await?;
        info!("broadcasted the withdrawal pub nonce message");

        // Wait for all the pub nonces to be broadcasted by other operators.
        // Collect all nonces.
        // Then signing will not fail.
        loop {
            debug!("trying to get all pub nonces from the bridge transaction database, waiting for other operators' nonces");
            let got_all_nonces = sig_manager
                .get_tx_state(&txid)
                .await
                .map_err(|e| WithdrawalExecError::TxState(e.to_string()))?
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
            .map_err(|e| WithdrawalExecError::Signing(e.to_string()))?;

        info!(%txid, "added own operator's partial signature");

        // if the flag is true, then the PSBT is fully signed by all required operators
        if flag {
            info!(%txid, "withdrawal transaction fully signed");
        }

        Ok(txid)
    } else {
        Err(WithdrawalExecError::Xpriv)
    }
}

/// Aggregate the received signature with the ones already accumulated.
///
/// This is executed by the bridge operator that is assigned the given withdrawal.
// TODO: pass in a database client once the database traits have been implemented.
pub async fn aggregate_withdrawal_sig(
    _withdrawal_info: &CooperativeWithdrawalInfo,
    _sig: &OperatorPartialSig,
) -> WithdrawalExecResult<Option<Signature>> {
    // setup logging
    let span = span!(
        Level::INFO,
        "starting withdrawal transaction signature aggregation"
    );
    let _guard = span.enter();

    // aggregates using MuSig2 the OperatorPartialSig into the BridgeStateOps
    // checks if is fully complete
    let mut tx_state = get_tx_state_by_txid(db_ops, txid).await?;

    event!(
        Level::DEBUG,
        event = "got an updated transaction state",
        %txid,
        ?tx_state
    );

    let is_fully_signed = tx_state
        .add_signature(*sig)
        .map_err(|e| WithdrawalExecError::Execution(e.to_string()))?;

    event!(
        Level::INFO,
        event = "transaction is or isn't fully signed",
        %is_fully_signed
    );

    if is_fully_signed {
        // get a new up-to-date transaction state
        let tx_state = get_tx_state_by_txid(db_ops, txid).await?;
        let sig = tx_state
            .aggregate_signature()
            .map_err(|e| WithdrawalExecError::Execution(e.to_string()))?;

        event!(
            Level::INFO,
            event = "aggregated final signature",
            %sig
        );

        Ok(Some(sig))
    } else {
        event!(
            Level::WARN,
            event = "could not aggregate final signature, missing partial sigs",
        );
        Ok(None)
    }
}
