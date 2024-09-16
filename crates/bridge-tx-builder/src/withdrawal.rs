//! Provides types/traits associated with the withdrawal process.

use alpen_express_primitives::{
    bridge::{OperatorIdx, TxSigningData},
    l1::{BitcoinPsbt, XOnlyPk},
};
use bitcoin::{Amount, FeeRate, OutPoint, Psbt, Transaction, TxOut};
use serde::{Deserialize, Serialize};

use crate::{
    context::{BuildContext, TxBuildContext},
    errors::{BridgeTxBuilderResult, CooperativeWithdrawalError},
    prelude::{
        anyone_can_spend_txout, create_taproot_addr, create_tx, create_tx_ins, create_tx_outs,
        metadata_script, SpendPath, BRIDGE_DENOMINATION, MIN_RELAY_FEE, OPERATOR_FEE,
    },
    TxKind,
};

/// Details for a withdrawal info assigned to an operator.
///
/// It has all the information required to create a transaction for fulfilling a user's withdrawal
/// request and pay operator fees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooperativeWithdrawalInfo {
    /// The [`OutPoint`] of the UTXO in the Bridge Address that is to be used to service the
    /// withdrawal request.
    deposit_outpoint: OutPoint,

    /// The x-only public key of the user used to create the taproot address that the user can
    /// spend from.
    user_pk: XOnlyPk,

    /// The index of the operator that is assigned the withdrawal.
    assigned_operator_idx: OperatorIdx,
}

impl TxKind for CooperativeWithdrawalInfo {
    type Context = TxBuildContext;

    fn construct_signing_data(
        &self,
        build_context: &Self::Context,
    ) -> BridgeTxBuilderResult<TxSigningData> {
        let prevout = self.create_prevout(build_context)?;
        let unsigned_tx = self.create_unsigned_tx(build_context, prevout.value)?;

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        psbt.inputs
            .get_mut(0)
            .expect("withdrawal tx is guaranteed to have one UTXO -- the deposit")
            .witness_utxo = Some(prevout);

        let psbt = BitcoinPsbt::from(psbt);

        Ok(TxSigningData {
            psbt,
            spend_infos: vec![None],
        })
    }
}

impl CooperativeWithdrawalInfo {
    /// Create a new withdrawal request.
    pub fn new(
        deposit_outpoint: OutPoint,
        user_pk: XOnlyPk,
        assigned_operator_idx: OperatorIdx,
    ) -> Self {
        Self {
            deposit_outpoint,
            user_pk,
            assigned_operator_idx,
        }
    }

    fn create_prevout<T: BuildContext>(&self, build_context: &T) -> BridgeTxBuilderResult<TxOut> {
        let dummy_el_address = &[0u8; 20];
        let metadata_script = metadata_script(dummy_el_address);
        let metadata_amount = metadata_script.to_p2wsh().minimal_non_dust();

        let anyone_can_spend_output_amount = anyone_can_spend_txout().value;

        // Finally, create the `TxOut` that sends user funds to the bridge multisig
        let fee_rate =
            FeeRate::from_sat_per_vb(MIN_RELAY_FEE.to_sat()).expect("invalid MIN_RELAY_FEE set");

        // We are not committing to any script path as the internal key should already be
        // randomized due to MuSig2 aggregation. See: <https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-23>
        let spend_path = SpendPath::KeySpend {
            internal_key: build_context.aggregated_pubkey(),
        };

        let (bridge_addr, _) = create_taproot_addr(build_context.network(), spend_path)?;

        let script_pubkey = bridge_addr.script_pubkey();
        let bridge_in_relay_cost = script_pubkey.minimal_non_dust_custom(fee_rate);

        let value = Amount::from(BRIDGE_DENOMINATION)
            - bridge_in_relay_cost
            - metadata_amount
            - anyone_can_spend_output_amount;

        Ok(TxOut {
            value,
            script_pubkey,
        })
    }

    fn create_unsigned_tx<T: BuildContext>(
        &self,
        build_context: &T,
        total_amount: Amount,
    ) -> BridgeTxBuilderResult<Transaction> {
        let tx_ins = create_tx_ins([self.deposit_outpoint]);

        // create the output for the operator fees
        let x_only_pubkey = build_context.pubkey().x_only_public_key().0;
        let spend_path = SpendPath::KeySpend {
            internal_key: x_only_pubkey,
        };

        let (operator_addr, _) = create_taproot_addr(build_context.network(), spend_path)?;

        // create the `anyone can spend` output for CPFP
        let anyone_can_spend_out = anyone_can_spend_txout();

        // create the output that pays to the user
        let user_addr = self
            .user_pk
            .to_address(*build_context.network())
            .map_err(CooperativeWithdrawalError::InvalidUserPk)?;
        let user_script_pubkey = user_addr.script_pubkey();

        let fee_rate = FeeRate::from_sat_per_vb(MIN_RELAY_FEE.to_sat())
            .expect("MIN_RELAY_FEE should be set correctly");
        let tx_fee = user_script_pubkey.minimal_non_dust_custom(fee_rate);

        let net_amount = total_amount - OPERATOR_FEE.into() - anyone_can_spend_out.value - tx_fee;

        let tx_outs = create_tx_outs([
            (user_script_pubkey, net_amount), // payout to the user
            (operator_addr.script_pubkey(), OPERATOR_FEE.into()), // operator fees
            // anyone can spend for CPFP
            (
                anyone_can_spend_out.script_pubkey,
                anyone_can_spend_out.value,
            ),
        ]);

        let unsigned_tx = create_tx(tx_ins, tx_outs);

        Ok(unsigned_tx)
    }
}
