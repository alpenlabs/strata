//! Builders related to building deposit-related transactions.
//!
//! Contains types, traits and implementations related to creating various transactions used in the
//! bridge-in dataflow.

use alpen_express_primitives::{
    bridge::TxSigningData,
    l1::{BitcoinAddress, BitcoinPsbt, SpendInfo},
};
use bitcoin::{
    key::TapTweak,
    secp256k1::SECP256K1,
    taproot::{self, ControlBlock},
    Address, Amount, FeeRate, OutPoint, Psbt, TapNodeHash, Transaction, TxOut,
};
use serde::{Deserialize, Serialize};

use super::{
    constants::{MIN_RELAY_FEE, UNSPENDABLE_INTERNAL_KEY},
    errors::{BridgeTxBuilderResult, DepositTransactionError},
    operations::{
        anyone_can_spend_txout, create_tx_ins, create_tx_outs, metadata_script, n_of_n_script,
    },
    TxKind,
};
use crate::{
    constants::BRIDGE_DENOMINATION,
    context::BuildContext,
    operations::create_tx,
    prelude::{create_taproot_addr, SpendPath},
};

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction outpoints from the users.
    deposit_request_outpoint: OutPoint,

    /// The execution layer address to mint the equivalent tokens to.
    /// As of now, this is just the 20-byte EVM address.
    el_address: Vec<u8>,

    /// The amount in bitcoins that the user wishes to deposit.
    total_amount: Amount,

    /// The hash of the take back leaf in the Deposit Request Transaction (DRT) as provided by the
    /// user in their `OP_RETURN` output.
    take_back_leaf_hash: TapNodeHash,

    /// The original taproot address in the Deposit Request Transaction (DRT) output used to
    /// sanity check computation internally i.e., whether the known information (n/n script spend
    /// path, [`UNSPENDABLE_INTERNAL_KEY`]) + the [`Self::take_back_leaf_hash`] yields the same
    /// P2TR address.
    original_taproot_addr: BitcoinAddress,
}

impl TxKind for DepositInfo {
    fn construct_signing_data<C: BuildContext>(
        &self,
        build_context: &C,
    ) -> BridgeTxBuilderResult<TxSigningData> {
        let prevouts = self.compute_prevouts(build_context)?;
        let spend_infos = self.compute_spend_infos(build_context)?;
        let unsigned_tx = self.create_unsigned_tx(build_context)?;

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;

        for (i, input) in psbt.inputs.iter_mut().enumerate() {
            input.witness_utxo = Some(prevouts[i].clone());
        }

        let psbt = BitcoinPsbt::from(psbt);

        Ok(TxSigningData { psbt, spend_infos })
    }
}

impl DepositInfo {
    /// Create a new deposit info with all the necessary data required to create a deposit
    /// transaction.
    pub fn new(
        deposit_request_outpoint: OutPoint,
        el_address: Vec<u8>,
        total_amount: Amount,
        take_back_leaf_hash: TapNodeHash,
        original_taproot_addr: BitcoinAddress,
    ) -> Self {
        Self {
            deposit_request_outpoint,
            el_address,
            total_amount,
            take_back_leaf_hash,
            original_taproot_addr,
        }
    }

    /// Get the total deposit amount that needs to be bridged-in.
    pub fn total_amount(&self) -> &Amount {
        &self.total_amount
    }

    /// Get the address in EL to mint tokens to.
    pub fn el_address(&self) -> &[u8] {
        &self.el_address
    }

    /// Get the outpoint of the Deposit Request Transaction (DRT) that is to spent in the Deposit
    /// Transaction (DT).
    pub fn deposit_request_outpoint(&self) -> &OutPoint {
        &self.deposit_request_outpoint
    }

    /// Get the hash of the user-takes-back leaf in the taproot of the Deposit Request Transaction
    /// (DRT).
    pub fn take_back_leaf_hash(&self) -> &TapNodeHash {
        &self.take_back_leaf_hash
    }

    fn compute_spend_infos(
        &self,
        build_context: &impl BuildContext,
    ) -> BridgeTxBuilderResult<Vec<Option<SpendInfo>>> {
        // The Deposit Request (DT) spends the n-of-n multisig leaf
        let spend_script = n_of_n_script(&build_context.aggregated_pubkey());
        let spend_script_hash =
            TapNodeHash::from_script(&spend_script, taproot::LeafVersion::TapScript);

        let takeback_script_hash = self.take_back_leaf_hash();

        let merkle_root = TapNodeHash::from_node_hashes(spend_script_hash, *takeback_script_hash);

        let address = Address::p2tr(
            SECP256K1,
            *UNSPENDABLE_INTERNAL_KEY,
            Some(merkle_root),
            *build_context.network(),
        );

        let expected_addr = self
            .original_taproot_addr
            .address()
            .clone()
            .require_network(*build_context.network())
            .map_err(|_e| DepositTransactionError::InvalidDRTAddress)?;

        if address != expected_addr {
            return Err(DepositTransactionError::InvalidTapLeafHash)?;
        }

        let (output_key, parity) = UNSPENDABLE_INTERNAL_KEY.tap_tweak(SECP256K1, Some(merkle_root));

        let control_block = ControlBlock {
            leaf_version: taproot::LeafVersion::TapScript,
            internal_key: *UNSPENDABLE_INTERNAL_KEY,
            merkle_branch: vec![*takeback_script_hash]
                .try_into()
                .map_err(|_e| DepositTransactionError::InvalidTapLeafHash)?,
            output_key_parity: parity,
        };

        if !control_block.verify_taproot_commitment(SECP256K1, output_key.into(), &spend_script) {
            return Err(DepositTransactionError::ControlBlockError)?;
        }

        let spend_info = Some(SpendInfo {
            script_buf: spend_script,
            control_block,
        });

        Ok(vec![spend_info])
    }

    fn compute_prevouts(&self, builder: &impl BuildContext) -> BridgeTxBuilderResult<Vec<TxOut>> {
        let deposit_address = self
            .original_taproot_addr
            .address()
            .clone()
            .require_network(*builder.network())
            .map_err(|_e| DepositTransactionError::InvalidDRTAddress)?;

        Ok(vec![TxOut {
            script_pubkey: deposit_address.script_pubkey(),
            value: BRIDGE_DENOMINATION.into(),
        }])
    }

    fn create_unsigned_tx(
        &self,
        build_context: &impl BuildContext,
    ) -> BridgeTxBuilderResult<Transaction> {
        // First, create the inputs
        let outpoint = self.deposit_request_outpoint();
        let tx_ins = create_tx_ins([*outpoint]);

        // Then, create the outputs:

        // First, create the `OP_RETURN <el_address>` output
        let el_addr = self.el_address();
        let el_addr: &[u8; 20] = el_addr
            .try_into()
            .map_err(|_e| DepositTransactionError::InvalidElAddressSize(el_addr.len()))?;

        let metadata_script = metadata_script(el_addr);
        let metadata_script_pubkey = metadata_script.to_p2wsh();
        let metadata_amount = metadata_script_pubkey.minimal_non_dust();

        // Then, create an `anyone-can-spend` output that can be used to confirm this transaction by
        // anyone using a CPFP strategy.
        let anyone_can_spend_output = anyone_can_spend_txout();

        // Finally, create the `TxOut` that sends user funds to the bridge multisig
        let fee_rate =
            FeeRate::from_sat_per_vb(MIN_RELAY_FEE.to_sat()).expect("invalid MIN_RELAY_FEE set");

        let spend_path = SpendPath::KeySpend {
            internal_key: build_context.aggregated_pubkey(),
        };

        let (bridge_addr, _) = create_taproot_addr(build_context.network(), spend_path)?;

        let bridge_in_script_pubkey = bridge_addr.script_pubkey();
        let bridge_in_relay_cost = bridge_in_script_pubkey.minimal_non_dust_custom(fee_rate);

        let net_bridge_in_amount = *self.total_amount()
            - bridge_in_relay_cost
            - metadata_amount
            - anyone_can_spend_output.value;
        dbg!(net_bridge_in_amount);

        let tx_outs = create_tx_outs([
            (bridge_in_script_pubkey, net_bridge_in_amount),
            (metadata_script_pubkey, metadata_amount),
            (
                anyone_can_spend_output.script_pubkey,
                anyone_can_spend_output.value,
            ),
        ]);

        let unsigned_tx = create_tx(tx_ins, tx_outs);

        Ok(unsigned_tx)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alpen_express_primitives::{bridge::PublickeyTable, buf::Buf20};
    use alpen_test_utils::bridge::{generate_keypairs, generate_pubkey_table};
    use bitcoin::{
        hashes::{sha256, Hash},
        hex::{Case, DisplayHex},
        secp256k1::Secp256k1,
        taproot::{self, TaprootBuilder},
        Address, Network,
    };

    use super::*;
    use crate::{
        constants::BRIDGE_DENOMINATION, context::TxBuildContext, errors::BridgeTxBuilderError,
        prelude::get_aggregated_pubkey,
    };

    #[test]
    fn test_create_spend_infos() {
        let (operator_pubkeys, _) = generate_keypairs(10);
        let operator_pubkeys = generate_pubkey_table(&operator_pubkeys);

        let deposit_request_outpoint = OutPoint::null();

        let (drt_output_address, take_back_leaf_hash) =
            create_drt_taproot_output(operator_pubkeys.clone());
        let self_index = 0;

        let tx_builder = TxBuildContext::new(operator_pubkeys, Network::Regtest, self_index);

        // Correct merkle proof
        let deposit_info = DepositInfo::new(
            deposit_request_outpoint,
            Buf20::default().0 .0.to_vec(),
            BRIDGE_DENOMINATION.into(),
            take_back_leaf_hash,
            drt_output_address.clone().into(),
        );

        let result = deposit_info.compute_spend_infos(&tx_builder);
        assert!(
            result.is_ok(),
            "should build the prevout for DT from the deposit info, error: {:?}",
            result.err().unwrap()
        );

        // Handles incorrect merkle proof
        let random_hash = sha256::Hash::hash(b"random_hash")
            .to_byte_array()
            .to_hex_string(Case::Lower);
        let deposit_info = DepositInfo::new(
            deposit_request_outpoint,
            Buf20::default().0 .0.to_vec(),
            BRIDGE_DENOMINATION.into(),
            TapNodeHash::from_str(&random_hash).unwrap(),
            drt_output_address.clone().into(),
        );

        let result = deposit_info.compute_spend_infos(&tx_builder);

        assert!(result.as_ref().err().is_some());
        assert!(
            matches!(
                result.unwrap_err(),
                BridgeTxBuilderError::DepositTransaction(
                    DepositTransactionError::InvalidTapLeafHash
                ),
            ),
            "should handle the case where the supplied merkle proof is wrong"
        );
    }

    fn create_drt_taproot_output(pubkeys: PublickeyTable) -> (Address, TapNodeHash) {
        let aggregated_pubkey = get_aggregated_pubkey(pubkeys);
        let n_of_n_spend_script = n_of_n_script(&aggregated_pubkey);

        // in actual DRT, this will be the take-back leaf.
        // for testing, this could be any script as we only care about its hash.
        let op_return_script = metadata_script(&Buf20::default().0 .0);
        let op_return_script_hash =
            TapNodeHash::from_script(&op_return_script, taproot::LeafVersion::TapScript);

        let taproot_builder = TaprootBuilder::new()
            .add_leaf(1, n_of_n_spend_script.clone())
            .unwrap()
            .add_leaf(1, op_return_script)
            .unwrap();

        let secp = Secp256k1::new();
        let spend_info = taproot_builder
            .finalize(&secp, *UNSPENDABLE_INTERNAL_KEY)
            .unwrap();

        (
            Address::p2tr(
                &secp,
                *UNSPENDABLE_INTERNAL_KEY,
                spend_info.merkle_root(),
                Network::Regtest,
            ),
            op_return_script_hash,
        )
    }
}
