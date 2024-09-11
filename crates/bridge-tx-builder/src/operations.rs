//! Provides some common, standalone utilities and wrappers over [`bitcoin`](bitcoin) to create
//! scripts, addresses and transactions.

use alpen_express_primitives::bridge::PublickeyTable;
use bitcoin::{
    absolute::LockTime,
    key::{Secp256k1, UntweakedPublicKey},
    opcodes::all::{OP_CHECKSIG, OP_PUSHBYTES_10, OP_PUSHBYTES_20, OP_PUSHNUM_1, OP_RETURN},
    script::Builder,
    secp256k1::{All, PublicKey, XOnlyPublicKey},
    taproot::{TaprootBuilder, TaprootSpendInfo},
    transaction, Address, Amount, Network, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Witness,
};
use musig2::{self, KeyAggContext};

use super::{
    constants::{MAGIC_BYTES, UNSPENDABLE_INTERNAL_KEY},
    errors::BridgeTxBuilderError,
};

/// Create a script with the spending condition that all signatures corresponding to the pubkey set
/// must be provided in (reverse) order.
pub fn n_of_n_script(aggregated_pubkey: &XOnlyPublicKey) -> ScriptBuf {
    Builder::new()
        .push_x_only_key(aggregated_pubkey)
        .push_opcode(OP_CHECKSIG)
        .into_script()
}

/// Aggregate the pubkeys using [`musig2`] and return the resulting [`XOnlyPublicKey`].
pub fn get_aggregated_pubkey(pubkeys: PublickeyTable) -> XOnlyPublicKey {
    let key_agg_ctx =
        KeyAggContext::new(pubkeys.0.values().copied()).expect("key aggregation of musig2 pubkeys");

    let aggregated_pubkey: PublicKey = key_agg_ctx.aggregated_pubkey();

    aggregated_pubkey.x_only_public_key().0
}

/// Create the metadata script that "stores" the execution layer address information.
pub fn metadata_script(el_address: &[u8; 20]) -> ScriptBuf {
    Builder::new()
        .push_opcode(OP_RETURN)
        .push_opcode(OP_PUSHBYTES_10)
        .push_slice(MAGIC_BYTES)
        .push_opcode(OP_PUSHBYTES_20)
        .push_slice(el_address)
        .into_script()
}

/// Create a taproot address for the given `scripts` and `internal_key`.
///
/// If the `scripts` is empty and some internal key is provided, a taproot address with only
/// key path spending is created.
///
/// And if an internal key is not provided, an [`UNSPENDABLE_INTERNAL_KEY`] is used to create a
/// taproot address with only script path spending.
///
/// # Errors
///
/// If the scripts is empty and the internal key is not provided (this would result in an
/// unspendable taproot address).
pub fn create_taproot_addr(
    secp: &Secp256k1<All>,
    network: &Network,
    scripts: &[ScriptBuf],
    internal_key: Option<UntweakedPublicKey>,
) -> Result<(Address, TaprootSpendInfo), BridgeTxBuilderError> {
    // there are no leaves in the taproot and there is no internal key either, it is invalid
    if scripts.is_empty() && internal_key.is_none() {
        return Err(BridgeTxBuilderError::EmptyTapscript);
    }

    let mut taproot_builder = TaprootBuilder::new();

    let internal_key = internal_key.unwrap_or(*UNSPENDABLE_INTERNAL_KEY);

    if scripts.is_empty() {
        // We are not committing to any script path as the internal key should already be randomized
        // due to MuSig aggregation. See: <https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-23>
        let spend_info = taproot_builder.finalize(secp, internal_key)?;

        return Ok((
            Address::p2tr(secp, internal_key, None, *network),
            spend_info,
        ));
    }

    let num_scripts = scripts.len();

    // compute depth for the taproot
    let max_depth = if num_scripts > 1 {
        (num_scripts - 1).ilog2() + 1
    } else {
        0
    };

    // max scripts if all the nodes are filled
    let max_num_scripts = 2usize.pow(max_depth);

    // number of scripts that exist at the penulimate level
    let num_penultimate_scripts = max_num_scripts - num_scripts;

    // number of scripts that exist at the deepest level
    let num_deepest_scripts = num_scripts - num_penultimate_scripts;

    for (script_idx, script) in scripts.iter().enumerate() {
        let depth = if script_idx < num_deepest_scripts {
            max_depth as u8
        } else {
            // if the deepest node is not filled, use the node at the upper level instead
            (max_depth - 1) as u8
        };

        taproot_builder = taproot_builder.add_leaf(depth, script.clone())?;
    }

    let spend_info = taproot_builder.finalize(secp, internal_key)?;

    Ok((
        Address::p2tr(
            secp,
            *UNSPENDABLE_INTERNAL_KEY,
            spend_info.merkle_root(),
            *network,
        ),
        spend_info,
    ))
}

/// Create an output that can be spent by anyone i.e, its script contains `OP_TRUE`.
pub fn anyone_can_spend_txout() -> TxOut {
    // `OP_PUSHNUM_1` is `OP_TRUE` that is it is always yields true for any unlocking script.
    let script = Builder::new().push_opcode(OP_PUSHNUM_1).into_script();
    let script_pubkey = script.to_p2wsh();
    let value = script_pubkey.minimal_non_dust();

    TxOut {
        script_pubkey,
        value,
    }
}

/// Create a bitcoin [`Transaction`] for the given transaction inputs and outputs.
pub fn create_tx(tx_ins: Vec<TxIn>, tx_outs: Vec<TxOut>) -> Transaction {
    Transaction {
        version: transaction::Version(2),
        lock_time: LockTime::from_consensus(0),
        input: tx_ins,
        output: tx_outs,
    }
}

/// Create a list of [`TxIn`]'s from given [`OutPoint`]'s.
///
/// This wraps the [`OutPoint`] in a structure that includes a blank `witness`, a blank
/// `script_sig` and the `sequence` set to enable replace-by-fee with no locktime.
pub fn create_tx_ins(utxos: impl IntoIterator<Item = OutPoint>) -> Vec<TxIn> {
    let mut tx_ins = Vec::new();

    for utxo in utxos {
        tx_ins.push(TxIn {
            previous_output: utxo,
            sequence: bitcoin::transaction::Sequence::ENABLE_RBF_NO_LOCKTIME,
            script_sig: ScriptBuf::default(),
            witness: Witness::new(),
        });
    }

    tx_ins
}

/// Create a list of [`TxOut`]'s' based on pairs of scripts and corresponding amounts.
pub fn create_tx_outs(
    scripts_and_amounts: impl IntoIterator<Item = (ScriptBuf, Amount)>,
) -> Vec<TxOut> {
    let mut tx_outs: Vec<TxOut> = Vec::new();

    for (script, amount) in scripts_and_amounts {
        tx_outs.push(TxOut {
            script_pubkey: script,
            value: amount,
        })
    }

    tx_outs
}

#[cfg(test)]
mod tests {
    use bitcoin::{
        key::Keypair,
        secp256k1::{rand, SecretKey},
    };

    use super::*;

    #[test]
    fn test_create_taproot_addr() {
        // create a bunch of dummy scripts to add to the taptree
        let max_scripts = 10;
        let scripts: Vec<ScriptBuf> = vec![ScriptBuf::from_bytes(vec![2u8; 32]); max_scripts];

        let network = Network::Regtest;
        let secp = Secp256k1::new();

        assert!(
            create_taproot_addr(&secp, &network, &[], None)
                .is_err_and(|x| matches!(x, BridgeTxBuilderError::EmptyTapscript)),
            "should error if there are no scripts and no internal key provided"
        );

        assert!(
            create_taproot_addr(&secp, &network, &scripts[0..1], None).is_ok(),
            "should work if the number of scripts is exactly 1 i.e., only root node exists"
        );

        assert!(
            create_taproot_addr(&secp, &network, &scripts[0..4], None).is_ok(),
            "should work if the number of scripts is an exact power of 2"
        );

        assert!(
            create_taproot_addr(&secp, &network, &scripts[..], None).is_ok(),
            "should work if the number of scripts is not an exact power of 2"
        );

        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let (x_only_public_key, _) = XOnlyPublicKey::from_keypair(&keypair);

        assert!(
            create_taproot_addr(&secp, &network, &[], Some(x_only_public_key)).is_ok(),
            "should support empty scripts with some internal key"
        );
    }
}
