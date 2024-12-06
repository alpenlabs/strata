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
//! - `m/56'/20'/100` for the message signing key
//! - `m/56'/20'/101` for the wallet transaction signing key

use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv, Xpub};
use secp256k1::SECP256K1;
#[cfg(feature = "zeroize")]
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::KeyError;

/// The base index for operator keys.
const BASE_IDX: u32 = 56;

/// The operator index for operator keys.
const OPERATOR_IDX: u32 = 20;

/// The message index for the operator message key.
const MESSAGE_IDX: u32 = 100;

/// The wallet index for the operator wallet key.
const WALLET_IDX: u32 = 101;

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
            ChildNumber::from_hardened_idx(BASE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(OPERATOR_IDX).unwrap(),
            ChildNumber::from_normal_idx(MESSAGE_IDX).unwrap(),
        ]);

        let wallet_path = DerivationPath::master().extend([
            ChildNumber::from_hardened_idx(BASE_IDX).unwrap(),
            ChildNumber::from_hardened_idx(OPERATOR_IDX).unwrap(),
            ChildNumber::from_normal_idx(WALLET_IDX).unwrap(),
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

#[cfg(feature = "zeroize")]
impl Zeroize for OperatorKeys {
    fn zeroize(&mut self) {
        let Self {
            master,
            signing,
            wallet,
        } = self;

        // # Security note
        //
        // Going over all possible "zeroizable" fields.
        // What we cannot zeroize is:
        //
        // - Network
        // - Child number
        //
        // These are fine to leave as they are since they are public parameters,
        // and not secret values.

        master.depth.zeroize();
        let mut parent_fingerprint: [u8; 4] = *master.parent_fingerprint.as_mut();
        parent_fingerprint.zeroize();
        master.private_key.non_secure_erase();
        let mut chaincode: [u8; 32] = *master.chain_code.as_mut();
        chaincode.zeroize();

        signing.depth.zeroize();
        let mut parent_fingerprint: [u8; 4] = *signing.parent_fingerprint.as_mut();
        parent_fingerprint.zeroize();
        signing.private_key.non_secure_erase();
        let mut chaincode: [u8; 32] = *signing.chain_code.as_mut();
        chaincode.zeroize();

        wallet.depth.zeroize();
        let mut parent_fingerprint: [u8; 4] = *wallet.parent_fingerprint.as_mut();
        parent_fingerprint.zeroize();
        wallet.private_key.non_secure_erase();
        let mut chaincode: [u8; 32] = *wallet.chain_code.as_mut();
        chaincode.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl ZeroizeOnDrop for OperatorKeys {}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::LazyLock};

    use bitcoin::{
        absolute, consensus, hashes::Hash, psbt::Input, transaction::Version, Address, Amount,
        OutPoint, Psbt, Sequence, TapSighashType, Transaction, TxIn, TxOut, Txid, Witness,
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
    const DEST_ADDRESS: &str = "bcrt1p5uhmu40t5yl97kr95s2m4sr8a9f3af2meqeefkx33symwex3wfqqfe77m3";

    // Dummy values for the test.
    const DUMMY_UTXO_AMOUNT: Amount = Amount::from_sat(20_000_000);
    const SPEND_AMOUNT: Amount = Amount::from_sat(19_999_000); // 1000 sat fee.

    #[test]
    fn test_operator_keys() {
        // Parse stuff
        let address = ADDRESS.parse::<Address<_>>().unwrap().assume_checked();
        let dest_address = DEST_ADDRESS.parse::<Address<_>>().unwrap().assume_checked();

        // Create the operator keys
        let operator_keys = OperatorKeys::new(&XPRIV).unwrap();
        let wallet_key = operator_keys.wallet_xpriv();
        let wallet_pubkey = operator_keys.wallet_xpub();
        let wallet_fingerprint = wallet_pubkey.fingerprint();
        let derivation_path = DerivationPath::master();
        let (x_only_pubkey, _) = wallet_pubkey.public_key.x_only_public_key();

        // Create a dummy transaction with a single input and output.
        let outpoint = OutPoint {
            txid: Txid::all_zeros(),
            vout: 0,
        };
        let txin = TxIn {
            previous_output: outpoint,
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            ..Default::default()
        };
        let txout = TxOut {
            value: SPEND_AMOUNT,
            script_pubkey: dest_address.script_pubkey(),
        };
        let previous_txout = TxOut {
            value: DUMMY_UTXO_AMOUNT,
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
    }

    #[cfg(feature = "zeroize")]
    #[test]
    fn test_zeroize() {
        let mut operator_keys = OperatorKeys::new(&XPRIV).unwrap();
        operator_keys.zeroize();
    }
}
