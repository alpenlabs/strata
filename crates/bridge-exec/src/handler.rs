//! Deposit/withdrawal transaction handling module

use std::{fmt::Debug, time::Duration};

use bitcoin::{key::Keypair, secp256k1::PublicKey, Transaction, Txid};
use borsh::{BorshDeserialize, BorshSerialize};
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

use crate::errors::{ExecError, ExecResult};

/// The interval that the bridge duty exec functions will poll for
/// bridge messages, such as public nonces and partial signatures.
// TODO: this is hardcoded, maybe move this to a user-configurable Config
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The execution context for handling bridge-related signing activities.
#[derive(Clone)]
pub struct ExecHandler<
    L2Client: StrataApiClient + Sync + Send,
    TxBuildContext: BuildContext + Sync + Send,
> {
    /// The build context required to create transaction data needed for signing.
    pub tx_build_ctx: TxBuildContext,

    /// The signature manager that handles nonce and signature aggregation.
    pub sig_manager: SignatureManager,

    /// The RPC client to connect to the RPC full node.
    pub l2_rpc_client: L2Client,

    /// The keypair for this client used to sign bridge-related messages.
    pub keypair: Keypair,

    /// This client's position in the MuSig2 signing ceremony.
    pub own_index: OperatorIdx,
}

impl<L2Client, TxBuildContext> ExecHandler<L2Client, TxBuildContext>
where
    L2Client: StrataApiClient + Sync + Send,
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
        info!("starting withdrawal transaction signing");

        let operator_pubkeys = self.tx_build_ctx.pubkey_table();
        let own_index = self.tx_build_ctx.own_index();
        let own_pubkey = operator_pubkeys
            .0
            .get(&own_index)
            .expect("could not find operator's pubkey in public key table");

        info!(
            ?tx_info,
            %self.own_index,
            %own_pubkey,
            "got the basic self information",
        );

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

    /// Add this client's own nonce and poll for nonces for a given [`Txid`] at [`POLL_INTERVAL`].
    pub async fn collect_nonces(&self, txid: &Txid) -> Result<(), ExecError> {
        let bitcoin_txid = BitcoinTxid::from(*txid);
        let public_nonce = self
            .sig_manager
            .get_own_nonce(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;

        let scope = Scope::V0PubNonce(bitcoin_txid);
        debug!(?scope, "create the withdrawal pub nonce scope");

        self.broadcast_msg(&scope, public_nonce, txid).await?;

        // TODO: use tokio::select to add a timeout path to prevent thread leaks.
        self.poll_for_nonces(scope, txid, POLL_INTERVAL).await?;

        Ok(())
    }

    async fn broadcast_msg<S: BorshSerialize + Debug>(
        &self,
        scope: &Scope,
        payload: S,
        txid: &Txid,
    ) -> Result<(), ExecError> {
        let message = MessageSigner::new(self.own_index, self.keypair.secret_key().into())
            .sign_scope(scope, &payload)
            .map_err(|e| ExecError::Signing(e.to_string()))?;
        debug!(?message, "created the message");

        let raw_message: Vec<u8> = message
            .try_into()
            .expect("could not serialize bridge message into raw bytes");
        self.l2_rpc_client
            .submit_bridge_msg(raw_message.into())
            .await?;

        info!(%txid, ?scope, "broadcasted message");
        Ok(())
    }

    /// Poll for nonces until all nonces have been collected.
    // TODO: use long-polling here instead.
    async fn poll_for_nonces(
        &self,
        scope: Scope,
        txid: &Txid,
        poll_interval: Duration,
    ) -> Result<(), ExecError> {
        debug!(%txid, "polling for other operators' nonces");

        loop {
            let received_nonces = self.parse_messages::<Musig2PubNonce>(scope.clone()).await?;

            let mut all_done = false;
            for pub_nonce in received_nonces {
                all_done = self
                    .sig_manager
                    .add_nonce(txid, self.own_index, &pub_nonce.1)
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

            sleep(poll_interval).await;
        }

        Ok(())
    }

    /// Add this client's own partial signature and poll for partial signatures from other clients
    /// for the given [`Txid`] at [`POLL_INTERVAL`].
    ///
    /// Once all the signatures are collected, this function also aggregates the signatures and
    /// creates the fully signed transaction.
    ///
    /// # Returns
    ///
    /// Fully signed transaction.
    pub async fn collect_signatures(&self, txid: &Txid) -> ExecResult<Transaction> {
        info!("starting transaction signature aggregation");

        let own_pubkey = self.get_own_pubkey();

        info!(
            %txid,
            %self.own_index,
            %own_pubkey,
            "got the basic self information",
        );

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

        self.broadcast_msg(&scope, partial_sig, txid).await?;

        // Wait for all the partial signatures to be broadcasted by other operators.
        // TODO: use tokio::select to add a timeout path to prevent thread leaks.
        self.poll_for_signatures(scope, txid, POLL_INTERVAL).await?;

        let tx = self
            .sig_manager
            .finalize_transaction(txid)
            .await
            .map_err(|e| ExecError::Signing(e.to_string()))?;
        info!(%txid, "transaction signature aggregation completed");

        Ok(tx)
    }

    // TODO: use long-polling here instead.
    async fn poll_for_signatures(
        &self,
        scope: Scope,
        txid: &Txid,
        poll_interval: Duration,
    ) -> Result<(), ExecError> {
        debug!("waiting for other operators' signatures");

        loop {
            let signatures = self
                .parse_messages::<Musig2PartialSig>(scope.clone())
                .await?;

            let mut all_signed = false;
            for partial_sig in signatures {
                let signature_info = OperatorPartialSig::new(partial_sig.1, partial_sig.0);
                all_signed = self
                    .sig_manager
                    .add_partial_sig(txid, signature_info)
                    .await
                    .map_err(|e| ExecError::Execution(e.to_string()))?;

                if all_signed {
                    break;
                }
            }

            if all_signed {
                info!("all signatues have been collected");
                break;
            }

            sleep(poll_interval).await;
        }

        Ok(())
    }

    fn get_own_pubkey(&self) -> PublicKey {
        let operator_pubkeys = self.tx_build_ctx.pubkey_table();
        let own_index = self.tx_build_ctx.own_index();
        *operator_pubkeys
            .0
            .get(&own_index)
            .expect("could not find operator's pubkey in public key table")
    }

    async fn parse_messages<Payload>(
        &self,
        scope: Scope,
    ) -> Result<impl Iterator<Item = (OperatorIdx, Payload)> + '_, ExecError>
    where
        Payload: BorshDeserialize,
    {
        let raw_scope: Vec<u8> = scope.clone().try_into().expect("serialization should work");

        let raw_scope: HexBytes = raw_scope.into();
        let received_payloads = self
            .l2_rpc_client
            .get_msgs_by_scope(raw_scope)
            .await?
            .into_iter()
            .filter_map(move |msg| {
                let msg = borsh::from_slice::<BridgeMessage>(&msg.0);
                if let Ok(msg) = msg {
                    let payload = msg.payload();
                    let payload = borsh::from_slice::<Payload>(payload);

                    if let Ok(payload) = payload {
                        Some((msg.source_id(), payload))
                    } else {
                        warn!(?scope, "skipping faulty message");
                        None
                    }
                } else {
                    warn!(?scope, "skipping faulty message");
                    None
                }
            });

        Ok(received_payloads)
    }
}

impl<L2Client, TxBuildContext> Debug for ExecHandler<L2Client, TxBuildContext>
where
    L2Client: StrataApiClient + Sync + Send,
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
