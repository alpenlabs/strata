//! Defines the `Execute` trait for the operator which encapsulates all bridge duties that an
//! operator must execute.

use async_trait::async_trait;

use alpen_express_state::bridge_duties::Duty;
use express_bridge_txm::{ReimbursementRequest, ValidateWithdrawal};

use crate::{
    config::{AddressConfig, Config},
    deposit_handler::HandleDeposit,
    withdrawal_handler::{errors::WithdrawalExecError, HandleWithdrawal},
};

use super::errors::ExecResult;

/// A meta trait that encapsulates all the traits that a bridge operator must implement and wires
/// them together.
#[async_trait]
pub trait Execute: HandleDeposit + HandleWithdrawal + ValidateWithdrawal {
    /// Execute the duty based on relevance and assignment.
    async fn execute(&self, duty: &Duty, config: &Config) -> ExecResult<()> {
        match duty {
            Duty::SignDeposit(deposit_request) => {
                // get the deposit info
                let deposit_info = deposit_request.deposit_info();

                // add one's own signature
                let sig = self.sign_deposit_tx(deposit_info).await?;

                // aggregate one's own signature with the one propagated from the network
                if let Ok(Some(agg_sig)) = self.aggregate_signature(deposit_request).await {
                    self.broadcast_deposit_tx(deposit_info, &agg_sig).await?;
                } else {
                    self.broadcast_partial_deposit_sig(deposit_info, &sig)
                        .await?;
                };

                Ok(())
            }

            Duty::FulfillWithdrawal(withdrawal_batch) => {
                if !self.is_assigned_to_me(withdrawal_batch).await {
                    return Ok(());
                }

                let AddressConfig {
                    address: reserved_addr,
                    network,
                } = config.reserved_addr.clone();

                let deposit_utxo = withdrawal_batch.deposit_utxo();
                let amount = withdrawal_batch.get_total_value();
                let source_utxo = self
                    .get_operator_utxo(reserved_addr.address().clone(), network, amount)
                    .await;

                let reimbursement_request =
                    ReimbursementRequest::new(source_utxo, *deposit_utxo, None);

                self.broadcast_reimbursement_request(&reimbursement_request)
                    .await
                    .map_err(|e| WithdrawalExecError::Broadcast(e.to_string()))?;

                Ok(())
            }

            Duty::SignWithdrawal(reimbursement_req) => {
                let validated_withdrawal = self
                    .validate_reimbursement_request(reimbursement_req)
                    .await?;

                let my_sig = self.sign_reimbursement_tx(&validated_withdrawal).await?;

                let signature_info = reimbursement_req.signature_info();

                // still unsigned so attach one's own signature
                if signature_info.is_none() {
                    self.broadcast_reimbursement_sig(&validated_withdrawal, &my_sig)
                        .await?;

                    return Ok(());
                }

                // aggregate the broadcasted signature with the ones already accumulated.
                let agg_sig = self
                    .aggregate_withdrawal_sig(
                        &validated_withdrawal,
                        signature_info.as_ref().unwrap(),
                    )
                    .await?;

                // if all the signatures have been aggregated, reimbursement tx can be broadcasted.
                if let Some(agg_sig) = agg_sig {
                    self.broadcast_withdrawal_tx(&validated_withdrawal, &agg_sig)
                        .await?;
                }

                // TODO: simplify the above logic to save bandwidth.
                // In the above naive implementation, the following events take place:
                // 1) Each client sees a request with no signature (after the assigned operator
                //    broadcasts it).
                // 2) Each operator, then attaches their own signature and broadcasts the signature
                //    along with the withdrawal and signature info.
                // 3) Each operator sees the signature propagated by themselves as well as others
                //    (assuming that the p2p logic is dumb and relays messages to all notes
                //    indiscriminately).
                // 4) Every time the operator sees a signature, it saves the signature to its
                //    database, and aggregates the signature with the ones already accumulated.
                // 5) Once all the signatures have been aggregated, the operator
                // This involves passing around a lot of additional information even ones that are
                // already available on the client (such as their own signature).

                Ok(())
            }
        }
    }
}
