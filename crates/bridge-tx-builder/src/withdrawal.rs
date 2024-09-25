//! Provides types/traits associated with the withdrawal process.

use alpen_express_primitives::{
    bridge::{OperatorIdx, TxSigningData},
    l1::{BitcoinPsbt, TaprootSpendPath, XOnlyPk},
};
use bitcoin::{Amount, FeeRate, OutPoint, Psbt, Transaction, TxOut};
use serde::{Deserialize, Serialize};

use crate::{
    context::BuildContext,
    errors::{BridgeTxBuilderResult, CooperativeWithdrawalError},
    prelude::{
        anyone_can_spend_txout, create_taproot_addr, create_tx, create_tx_ins, create_tx_outs,
        SpendPath, BRIDGE_DENOMINATION, MIN_RELAY_FEE, OPERATOR_FEE,
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
    fn construct_signing_data<C: BuildContext>(
        &self,
        build_context: &C,
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
            spend_path: TaprootSpendPath::Key,
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
        // We are not committing to any script path as the internal key should already be
        // randomized due to MuSig2 aggregation. See: <https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-23>
        let spend_path = SpendPath::KeySpend {
            internal_key: build_context.aggregated_pubkey(),
        };

        let (bridge_addr, _) = create_taproot_addr(build_context.network(), spend_path)?;

        Ok(TxOut {
            value: BRIDGE_DENOMINATION.into(),
            script_pubkey: bridge_addr.script_pubkey(),
        })
    }

    fn create_unsigned_tx<T: BuildContext>(
        &self,
        build_context: &T,
        total_amount: Amount,
    ) -> BridgeTxBuilderResult<Transaction> {
        let tx_ins = create_tx_ins([self.deposit_outpoint]);

        // create the output for the operator fees
        let pubkey_table = build_context.pubkey_table();
        let assigned_operator_pubkey = pubkey_table.0.get(&self.assigned_operator_idx);

        if assigned_operator_pubkey.is_none() {
            return Err(CooperativeWithdrawalError::Unauthorized(
                self.assigned_operator_idx,
            ))?;
        }

        let x_only_pubkey = assigned_operator_pubkey
            .expect("should be present")
            .x_only_public_key()
            .0;
        let spend_path = SpendPath::KeySpend {
            internal_key: x_only_pubkey,
        };

        let (operator_addr, _) = create_taproot_addr(build_context.network(), spend_path)?;

        // create the `anyone can spend` output for CPFP
        let anyone_can_spend_out = anyone_can_spend_txout();

        // create the output that pays to the user
        let user_addr = self
            .user_pk
            .to_p2tr_address(*build_context.network())
            .map_err(CooperativeWithdrawalError::InvalidUserPk)?;
        let user_script_pubkey = user_addr.script_pubkey();

        // This fee pays for the entire transaction.
        // In the current configuration of `10` for `MIN_RELAY_FEE`, the total transaction fee
        // computes to ~5.5 SAT (run integration tests with `RUST_LOG=warn` to verify).
        let fee_rate = FeeRate::from_sat_per_vb(MIN_RELAY_FEE.to_sat())
            .expect("MIN_RELAY_FEE should be set correctly");
        let tx_fee = user_script_pubkey.minimal_non_dust_custom(fee_rate);

        let net_amount = total_amount - OPERATOR_FEE - anyone_can_spend_out.value - tx_fee;

        let tx_outs = create_tx_outs([
            (user_script_pubkey, net_amount),              // payout to the user
            (operator_addr.script_pubkey(), OPERATOR_FEE), // operator fees
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

#[cfg(test)]
mod tests {
    use std::ops::Not;

    use alpen_express_primitives::{
        bridge::OperatorIdx,
        buf::Buf32,
        errors::ParseError,
        l1::{TaprootSpendPath, XOnlyPk},
    };
    use alpen_test_utils::bridge::{generate_keypairs, generate_pubkey_table};
    use bitcoin::{
        hashes::{sha256d, Hash},
        Amount, Network, OutPoint, Txid,
    };

    use crate::{
        context::TxBuildContext,
        errors::{BridgeTxBuilderError, CooperativeWithdrawalError},
        prelude::{CooperativeWithdrawalInfo, BRIDGE_DENOMINATION},
        TxKind,
    };

    #[test]
    fn test_construct_signing_data_success() {
        // Arrange
        let (pubkeys, _seckeys) = generate_keypairs(3);
        let pubkey_table = generate_pubkey_table(&pubkeys[..]);
        let deposit_outpoint =
            OutPoint::new(Txid::from_raw_hash(sha256d::Hash::hash(&[1u8; 32])), 1);

        let user_index = 0usize;
        let assigned_operator_idx = 2usize;
        assert_ne!(
            user_index, assigned_operator_idx,
            "use separate indexes for user and assigned operator"
        );

        let user_pk = XOnlyPk::new(Buf32(
            pubkeys[user_index].x_only_public_key().0.serialize().into(),
        ));

        let assigned_operator_idx = assigned_operator_idx as OperatorIdx;

        let withdrawal_info =
            CooperativeWithdrawalInfo::new(deposit_outpoint, user_pk, assigned_operator_idx);

        let build_context = TxBuildContext::new(
            Network::Regtest,
            pubkey_table,
            assigned_operator_idx as OperatorIdx,
        );

        // Act
        let signing_data_result = withdrawal_info.construct_signing_data(&build_context);

        // Assert
        assert!(
            signing_data_result.is_ok(),
            "should be able to construct TxSigningData"
        );
        let signing_data = signing_data_result.unwrap();

        // Verify that the PSBT has one input and three outputs as per create_unsigned_tx
        let psbt = signing_data.psbt.inner();
        assert_eq!(
            psbt.inputs.len(),
            1,
            "withdrawal psbt should have 1 input (the deposit)"
        );
        assert_eq!(
            psbt.outputs.len(),
            3,
            "withdrawal psbt should have 3 outputs -- payout, operator fee, and anybody takes"
        );

        assert!(
            matches!(signing_data.spend_path, TaprootSpendPath::Key),
            "signing data should have a keypath spend"
        );
    }

    #[test]
    fn test_construct_signing_data_invalid_user_pk() {
        // Arrange
        let (pubkeys, _seckeys) = generate_keypairs(2);
        let pubkey_table = generate_pubkey_table(&pubkeys[..]);
        let deposit_outpoint =
            OutPoint::new(Txid::from_raw_hash(sha256d::Hash::hash(&[2u8; 32])), 2);

        let user_index = 1usize;
        let assigned_operator_idx = 0usize;
        assert_ne!(
            user_index, assigned_operator_idx,
            "use separate indexes for user and assigned operator"
        );

        // Create an invalid XOnlyPublicKey by using an all-zero buffer
        let invalid_user_pk = XOnlyPk::new(Buf32::zero());
        let assigned_operator_idx = assigned_operator_idx as OperatorIdx;

        let withdrawal_info = CooperativeWithdrawalInfo::new(
            deposit_outpoint,
            invalid_user_pk,
            assigned_operator_idx,
        );

        let build_context =
            TxBuildContext::new(Network::Regtest, pubkey_table, assigned_operator_idx);

        // Act
        let signing_data_result = withdrawal_info.construct_signing_data(&build_context);

        // Assert
        assert!(signing_data_result.is_err_and(|e| matches!(
            e,
            BridgeTxBuilderError::CooperativeWithdrawalTransaction(
                CooperativeWithdrawalError::InvalidUserPk(ParseError::InvalidPubkey(_)),
            ),
        )));
    }

    #[test]
    fn test_create_prevout_success() {
        // Arrange
        let (pubkeys, _seckeys) = generate_keypairs(3);
        let pubkey_table = generate_pubkey_table(&pubkeys[..]);
        let deposit_outpoint =
            OutPoint::new(Txid::from_raw_hash(sha256d::Hash::hash(&[3u8; 32])), 3);

        let user_index = 1usize;
        let assigned_operator_idx = 0usize;
        assert_ne!(
            user_index, assigned_operator_idx,
            "use separate indexes for user and assigned operator"
        );

        let user_pk = XOnlyPk::new(Buf32(
            pubkeys[user_index].x_only_public_key().0.serialize().into(),
        ));
        let assigned_operator_idx = assigned_operator_idx as OperatorIdx;

        let withdrawal_info =
            CooperativeWithdrawalInfo::new(deposit_outpoint, user_pk, assigned_operator_idx);

        let build_context =
            TxBuildContext::new(Network::Regtest, pubkey_table, assigned_operator_idx);

        // Act
        let prevout_result = withdrawal_info.create_prevout(&build_context);

        // Assert
        assert!(prevout_result.is_ok());
        let prevout = prevout_result.unwrap();

        assert!(prevout.script_pubkey.is_empty().not());

        assert!(
            prevout.value.eq(&BRIDGE_DENOMINATION.into()),
            "output amount equal to the bridge denomination"
        );
    }

    #[test]
    fn test_create_unsigned_tx_success() {
        // Arrange
        let (pubkeys, _seckeys) = generate_keypairs(4);
        let pubkey_table = generate_pubkey_table(&pubkeys[..]);
        let deposit_outpoint =
            OutPoint::new(Txid::from_raw_hash(sha256d::Hash::hash(&[4u8; 32])), 4);

        let user_index = 3usize;
        let assigned_operator_idx = 0usize;
        assert_ne!(
            user_index, assigned_operator_idx,
            "use separate indexes for user and assigned operator"
        );

        let user_pk = XOnlyPk::new(Buf32(
            pubkeys[user_index].x_only_public_key().0.serialize().into(),
        ));
        let assigned_operator_idx = assigned_operator_idx as OperatorIdx;

        let withdrawal_info =
            CooperativeWithdrawalInfo::new(deposit_outpoint, user_pk, assigned_operator_idx);

        let build_context =
            TxBuildContext::new(Network::Regtest, pubkey_table, assigned_operator_idx);

        // Act
        let unsigned_tx_result =
            withdrawal_info.create_unsigned_tx(&build_context, Amount::from(BRIDGE_DENOMINATION));

        // Assert
        assert!(unsigned_tx_result.is_ok());
        let unsigned_tx = unsigned_tx_result.unwrap();

        // Verify that the transaction has the correct number of inputs and outputs
        assert_eq!(unsigned_tx.input.len(), 1);
        assert_eq!(unsigned_tx.output.len(), 3);
    }
}
