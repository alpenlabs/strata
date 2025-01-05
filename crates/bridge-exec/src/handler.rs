//! Deposit/withdrawal transaction handling module

use std::{fmt::Debug, time::Duration};

use bitcoin::{key::Keypair, Transaction, Txid};
use borsh::{BorshDeserialize, BorshSerialize};
use deadpool::managed::{Object, Pool};
use jsonrpsee::tokio::time::sleep;
use strata_bridge_sig_manager::manager::SignatureManager;
use strata_bridge_tx_builder::{context::BuildContext, TxKind};
use strata_primitives::{
    bridge::{Musig2PartialSig, Musig2PubNonce, OperatorIdx, OperatorPartialSig},
    l1::BitcoinTxid,
    relay::{
        types::{BridgeMessage, Scope},
        util::MessageSigner,
    },
};
use strata_rpc_api::StrataApiClient;
use strata_rpc_types::HexBytes;
use tracing::{debug, info, warn};

use crate::{
    errors::{ExecError, ExecResult},
    ws_client::WsClientManager,
};

/// WebSocket client pool
pub type WsClientPool = Pool<WsClientManager>;

/// The execution context for handling bridge-related signing activities.
#[derive(Clone)]
pub struct ExecHandler<TxBuildContext: BuildContext + Sync + Send> {
    /// The build context required to create transaction data needed for signing.
    pub tx_build_ctx: TxBuildContext,

    /// The signature manager that handles nonce and signature aggregation.
    pub sig_manager: SignatureManager,

    /// The RPC client to connect to the RPC full node.
    pub l2_rpc_client_pool: WsClientPool,

    /// The keypair for this client used to sign bridge-related messages.
    pub keypair: Keypair,

    /// This client's position in the MuSig2 signing ceremony.
    pub own_index: OperatorIdx,

    /// The interval for polling bridge messages.
    pub msg_polling_interval: Duration,
}

impl<TxBuildContext> ExecHandler<TxBuildContext>
where
    TxBuildContext: BuildContext + Sync + Send,
{
    /// Construct and sign a transaction based on the provided `TxInfo`.
    ///
    /// # Returns
    ///
    /// The transaction ID of the constructed transaction.
    pub async fn sign_tx<TxInfo>(&self, tx_info: TxInfo) -> ExecResult<Txid>
    where
        TxInfo: TxKind + Debug,
    {
        info!("starting transaction signing");

        let operator_pubkeys = self.tx_build_ctx.pubkey_table();

        info!(?tx_info, "received transaction details");

        // sign the transaction with MuSig2 and put inside the OperatorPartialSig

        // construct the transaction data
        let tx_signing_data = tx_info.construct_signing_data(&self.tx_build_ctx)?;

        debug!(?tx_signing_data, "got the signing data");

        // add the tx_state to the sig_manager in order to generate a sec_nonce and pub_nonce
        let txid = self
            .sig_manager
            .add_tx_state(tx_signing_data, operator_pubkeys.clone())
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;

        info!(
            %txid,
            "added the public nonce to the bridge transaction database",
        );

        Ok(txid)
    }

    /// Add this client's own nonce and poll for nonces for a given [`Txid`].
    pub async fn collect_nonces(&self, txid: &Txid) -> Result<(), ExecError> {
        let bitcoin_txid = BitcoinTxid::from(*txid);
        let public_nonce = self
            .sig_manager
            .get_own_nonce(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;

        let scope = Scope::V0PubNonce(bitcoin_txid);
        debug!(?scope, "created the pub nonce scope");

        let message = self.broadcast_msg(&scope, public_nonce, txid).await?;

        // TODO: use tokio::select to add a timeout path to prevent thread leaks.
        self.poll_for_nonces(message.scope(), txid).await?;

        Ok(())
    }

    async fn broadcast_msg<S: BorshSerialize + Debug>(
        &self,
        scope: &Scope,
        payload: S,
        txid: &Txid,
    ) -> Result<BridgeMessage, ExecError> {
        let signed_message = MessageSigner::new(self.own_index, self.keypair.secret_key().into())
            .sign_scope(scope, &payload)
            .map_err(|e| ExecError::Signing(e.to_string()))?;
        debug!(?signed_message, "created the message");

        let raw_message = borsh::to_vec::<BridgeMessage>(&signed_message)
            .expect("should be able to borsh serialize raw message");

        let l2_rpc_client = self.get_ready_rpc_client().await?;

        l2_rpc_client.submit_bridge_msg(raw_message.into()).await?;

        info!(%txid, ?scope, ?payload, "broadcasted message");
        Ok(signed_message)
    }

    /// Poll for nonces until all nonces have been collected.
    // TODO: use long-polling here instead.
    async fn poll_for_nonces(&self, scope: &[u8], txid: &Txid) -> Result<(), ExecError> {
        debug!(%txid, "polling for other operators' nonces");

        loop {
            let received_nonces = self.parse_messages::<Musig2PubNonce>(scope).await?;

            let mut all_done = false;
            for (sender_idx, pub_nonce) in received_nonces {
                all_done = self
                    .sig_manager
                    .add_nonce(txid, sender_idx, &pub_nonce)
                    .await
                    .map_err(|e| ExecError::Execution(e.to_string()))?;

                if all_done {
                    break;
                }
            }

            if all_done {
                info!("got all pub nonces from the bridge transaction database");
                break;
            }

            sleep(self.msg_polling_interval).await;
        }

        Ok(())
    }

    /// Add this client's own partial signature and poll for partial signatures from other clients
    /// for the given [`Txid`].
    ///
    /// Once all the signatures are collected, this function also aggregates the signatures and
    /// creates the fully signed transaction.
    ///
    /// # Returns
    ///
    /// Fully signed transaction.
    pub async fn collect_signatures(&self, txid: &Txid) -> ExecResult<Transaction> {
        info!(%txid, "starting transaction signature aggregation");
        let tx_state = self
            .sig_manager
            .get_tx_state(txid)
            .await
            .map_err(|e| ExecError::TxState(e.to_string()))?;

        debug!(
            %txid,
            ?tx_state,
            "got transaction state from bridge database",
        );

        // Fully signed and in the database, nothing to do here...
        if tx_state.is_fully_signed() {
            info!(
                %txid,
                "transaction already fully signed and in the database",
            );
            let tx = self
                .sig_manager
                .finalize_transaction(txid)
                .await
                .map_err(|e| ExecError::Signing(e.to_string()))?;

            return Ok(tx);
        }

        // First add this operator's own partial signature.
        self.sig_manager
            .add_own_partial_sig(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;
        info!(
            %txid,
            "added own's partial signature to the bridge transaction database",
        );

        // Now, get the added partial sig
        let partial_sig = self
            .sig_manager
            .get_own_partial_sig(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?
            .expect("should've been signed");

        info!(
            ?partial_sig,
            "got own partial signature from the bridge transaction database",
        );

        // submit_message RPC call
        let bitcoin_txid: BitcoinTxid = (*txid).into();

        let scope = Scope::V0Sig(bitcoin_txid);

        let message = self.broadcast_msg(&scope, partial_sig, txid).await?;

        // Wait for all the partial signatures to be broadcasted by other operators.
        // TODO: use tokio::select to add a timeout path to prevent thread leaks.
        self.poll_for_signatures(message.scope(), txid).await?;

        let tx = self
            .sig_manager
            .finalize_transaction(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;
        info!(%txid, "transaction signature aggregation completed");

        Ok(tx)
    }

    // TODO: use long-polling here instead.
    async fn poll_for_signatures(&self, scope: &[u8], txid: &Txid) -> Result<(), ExecError> {
        debug!("waiting for other operators' signatures");

        loop {
            let signatures = self.parse_messages::<Musig2PartialSig>(scope).await?;

            let mut all_signed = false;
            for (signer_index, partial_sig) in signatures {
                let signature_info = OperatorPartialSig::new(partial_sig, signer_index);
                let result = self.sig_manager.add_partial_sig(txid, signature_info).await;

                if let Err(e) = result {
                    warn!(err=%e, %signer_index, "discarding invalid signature");
                } else {
                    all_signed = result.expect("must never error");
                }

                if all_signed {
                    break;
                }
            }

            if all_signed {
                info!("all signatures have been collected");
                break;
            }

            sleep(self.msg_polling_interval).await;
        }

        Ok(())
    }

    async fn parse_messages<'parser, Payload>(
        &'parser self,
        scope: &'parser [u8],
    ) -> Result<impl Iterator<Item = (OperatorIdx, Payload)> + 'parser, ExecError>
    where
        Payload: BorshDeserialize + Debug,
    {
        let raw_scope: HexBytes = scope.into();
        info!(?scope, "getting messages from the L2 Client");

        let l2_rpc_client = self.get_ready_rpc_client().await?;

        let received_payloads = l2_rpc_client
            .get_msgs_by_scope(raw_scope)
            .await?
            .into_iter()
            .filter_map(move |msg| {
                let msg = borsh::from_slice::<BridgeMessage>(&msg.0);
                if let Ok(msg) = msg {
                    let raw_payload = msg.payload();
                    let payload = borsh::from_slice::<Payload>(raw_payload);
                    let raw_scope = msg.scope();
                    let scope = borsh::from_slice::<Scope>(raw_scope);
                    debug!(?msg, ?payload, ?scope, "got message from the L2 Client");

                    match payload {
                        Ok(payload) => Some((msg.source_id(), payload)),
                        Err(ref error) => {
                            warn!(?scope, ?payload, ?error, "skipping faulty message payload");
                            None
                        }
                    }
                } else {
                    warn!(?scope, "skipping faulty message");
                    None
                }
            });

        Ok(received_payloads)
    }

    /// Retrieves a ready-to-use RPC client from the client pool.
    pub async fn get_ready_rpc_client(&self) -> Result<Object<WsClientManager>, ExecError> {
        self.l2_rpc_client_pool
            .get()
            .await
            .map_err(|_| ExecError::WsPool)
    }
}

impl<TxBuildContext> Debug for ExecHandler<TxBuildContext>
where
    TxBuildContext: BuildContext + Sync + Send,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Handler Context index: {}, pubkey: {}",
            self.own_index,
            self.keypair.public_key()
        )
    }
}
