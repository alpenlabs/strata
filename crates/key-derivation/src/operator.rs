//! Key derivation for bridge operators
//!
//! Bridge operators guarantee the security assumptions of the Strata BitVM-based
//! bridge by enforcing that all peg-ins and peg-outs are valid.
//!
//! Operators are responsible for their own keys and master [`Xpriv`] is not
//! shared between operators. Hence, this crate has a BYOK (Bring Your Own Key) design.
//!
//! They use a set of keys to sign messages and bitcoin transactions.
//! The keys are derived from a master [`Xpriv`]
//! using a [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
//! HD derivation path.
//!
//! The derivation paths are:
//!
//! - `m/56'/20'/100'` for the message signing key
//! - `m/56'/20'/101'` for the wallet transaction signing key
//!
//! These follow [BIP-43](https://github.com/bitcoin/bips/blob/master/bip-0043.mediawiki)
//! and [BIP-44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
//! levels for bitcoin keys:
//!
//! - `m/56'` is the BIP-44 purpose level
//! - `m/56'/20'` is the coin type level
//! - `m/56'/20'/100'` is the account level
//!
//! These are all hardened child indices, allowing extra security if an operator derived
//! [`Xpub`](bitcoin::bip32::Xpub) is compromised.

use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    secp256k1::SECP256K1,
};

use crate::error::KeyError;

/// The BIP-44 "purpose" index for operator keys.
const PURPOSE_IDX: u32 = 56;

/// The BIP-44 "coin type" index for operator keys.
const COIN_TYPE_IDX: u32 = 20;

/// The BIP-44 "account" index for the operator message key.
const ACCOUNT_MESSAGE_IDX: u32 = 100;

/// The BIP-44 "account" index for the operator wallet key.
const ACCOUNT_WALLET_IDX: u32 = 101;

/// The operator's message signing and wallet transaction signing keys.
///
/// The keys have different [`Xpriv`] derivation paths to allow for different
/// key lifetimes, while adding some security against a leaked signing key.
#[derive(Debug, Clone)]
pub struct OperatorKeys {
    /// The operator's master [`Xpriv`].
    master: Xpriv,
    /// The operator's message signing [`Xpriv`].
    signing: Xpriv,
    /// The operator's wallet transaction signing [`Xpriv`].
    wallet: Xpriv,
}

impl OperatorKeys {
    /// Creates a new [`OperatorKeys`] from a master [`Xpriv`].
    pub fn new(master: &Xpriv) -> Result<Self, KeyError> {
        let message_path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(PURPOSE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(COIN_TYPE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(ACCOUNT_MESSAGE_IDX).unwrap(),
        ]);

        let wallet_path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(PURPOSE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(COIN_TYPE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(ACCOUNT_WALLET_IDX).unwrap(),
        ]);

        let message_xpriv = master.derive_priv(SECP256K1, &message_path)?;
        let wallet_xpriv = master.derive_priv(SECP256K1, &wallet_path)?;

        Ok(Self {
            master: *master,
            signing: message_xpriv,
            wallet: wallet_xpriv,
        })
    }

    /// Operator's master [`Xpriv`].
    pub fn master_xpriv(&self) -> Xpriv {
        self.master
    }

    /// Operator's wallet transaction signing [`Xpriv`].
    pub fn wallet_xpriv(&self) -> Xpriv {
        self.wallet
    }

    /// Operator's message signing [`Xpriv`].
    pub fn message_xpriv(&self) -> Xpriv {
        self.signing
    }

    /// Operator's master [`Xpub`].
    pub fn master_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.master)
    }

    /// Operator's message signing [`Xpub`].
    ///
    /// Infallible according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn message_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.signing)
    }

    /// Operator's wallet transaction signing [`Xpub`].
    ///
    /// Infallible conversion from [`Xpriv`] to [`Xpub`] according to
    /// [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
    pub fn wallet_xpub(&self) -> Xpub {
        Xpub::from_priv(SECP256K1, &self.wallet)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::LazyLock};

    use bitcoin::{
        absolute, consensus, psbt::Input, transaction::Version, Address, Amount, BlockHash,
        OutPoint, Psbt, Sequence, TapSighashType, Transaction, TxIn, TxOut, Witness,
    };

    use super::*;

    // What's better than bacon? bacon^24 of course
    // Thix xpriv was generated by the bacon^24 mnemonic
    // Don't use this in production!
    const XPRIV_STR: &str = "tprv8ZgxMBicQKsPeh9dSitM82FU7Fz3ZgPkKmmovAr2aqwauAMVgjcEkZBb2etBtRPZ8XYVm7shxcKwVaDus7T5kauJXVsqAfzM4Tty13rRjAG";
    static XPRIV: LazyLock<Xpriv> = LazyLock::new(|| XPRIV_STR.parse().unwrap());

    // The first address derived from the xpriv above using a `tr()` descriptor.
    const ADDRESS: &str = "bcrt1p729l9680ht3zf7uhl6pgdrlhfp9r29cwajr5jk3k05fer62763fscz0w4s";

    // The second address derived from the xpriv above using a `tr()` descriptor.
    const SENT_ADDRESS: &str = "bcrt1p5uhmu40t5yl97kr95s2m4sr8a9f3af2meqeefkx33symwex3wfqqfe77m3";

    #[test]
    fn test_operator_keys() {
        // Parse stuff
        let address = ADDRESS.parse::<Address<_>>().unwrap().assume_checked();
        let dest_address = SENT_ADDRESS.parse::<Address<_>>().unwrap().assume_checked();

        // Start a bitcoind node
        let bitcoind = corepc_node::BitcoinD::from_downloaded().unwrap();

        // Mine some blocks
        let blocks = bitcoind
            .client
            .generate_to_address(101, &address)
            .unwrap()
            .0;
        assert_eq!(blocks.len(), 101);

        // Mine more blocks
        let _ = bitcoind
            .client
            .generate_to_address(1, &dest_address)
            .unwrap()
            .0;

        // Create the operator keys
        let operator_keys = OperatorKeys::new(&XPRIV).unwrap();
        let wallet_key = operator_keys.wallet_xpriv();
        let wallet_pubkey = operator_keys.wallet_xpub();
        let wallet_fingerprint = wallet_pubkey.fingerprint();
        let derivation_path = DerivationPath::master();
        let (x_only_pubkey, _) = wallet_pubkey.public_key.x_only_public_key();

        // Get the coinbase of the last mined block.
        let block_hash: BlockHash = blocks.first().unwrap().parse().unwrap();
        let block = bitcoind.client.get_block(block_hash).unwrap();
        let coinbase_tx = block.txdata.last().unwrap();

        // Create a transaction with a single input and output.
        let txid = coinbase_tx.compute_txid();
        let outpoint = OutPoint::new(txid, 0);
        let txin = TxIn {
            previous_output: outpoint,
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            ..Default::default()
        };
        let txout = TxOut {
            value: Amount::from_btc(49.99).unwrap(),
            script_pubkey: dest_address.script_pubkey(),
        };
        let previous_txout = TxOut {
            value: Amount::from_btc(50.0).unwrap(),
            script_pubkey: address.script_pubkey(),
        };

        // Create the unsigned transaction
        let transaction = Transaction {
            version: Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![txout],
        };

        // Create the PSBT
        let mut psbt = Psbt::from_unsigned_tx(transaction).expect("could not create PSBT");
        let ty = TapSighashType::All.into();
        let origins = BTreeMap::from([(
            x_only_pubkey,
            (vec![], (wallet_fingerprint, derivation_path)),
        )]);

        // Add the input to the PSBT
        psbt.inputs = vec![Input {
            witness_utxo: Some(previous_txout),
            tap_key_origins: origins,
            tap_internal_key: Some(x_only_pubkey),
            sighash_type: Some(ty),
            ..Default::default()
        }];

        // Sign the PSBT
        psbt.sign(&wallet_key, SECP256K1)
            .expect("could not sign PSBT");

        // Finalize the PSBT
        psbt.inputs[0].final_script_witness = Some(Witness::p2tr_key_spend(
            &psbt.inputs[0].tap_key_sig.unwrap(),
        ));
        // Clear all the data fields as per the spec.
        psbt.inputs[0].partial_sigs = BTreeMap::new();
        psbt.inputs[0].sighash_type = None;
        psbt.inputs[0].redeem_script = None;
        psbt.inputs[0].witness_script = None;
        psbt.inputs[0].bip32_derivation = BTreeMap::new();

        // Extract the transaction and serialize it
        let signed_tx = psbt.extract_tx().expect("valid transaction");
        let serialized_signed_tx = consensus::encode::serialize_hex(&signed_tx);
        println!("serialized_signed_tx: {}", serialized_signed_tx);

        // Broadcast the transaction
        bitcoind.client.send_raw_transaction(&signed_tx).unwrap();
    }
}
