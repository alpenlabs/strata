//! Provides some common, standalone utilities and wrappers over [`bitcoin`] to create
//! scripts, addresses and transactions.

use bitcoin::{
    absolute::LockTime,
    key::UntweakedPublicKey,
    opcodes::{
        all::{OP_CHECKSIG, OP_RETURN},
        OP_TRUE,
    },
    script::{Builder, PushBytesBuf},
    secp256k1::{PublicKey, XOnlyPublicKey, SECP256K1},
    taproot::{TaprootBuilder, TaprootSpendInfo},
    transaction, Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut,
    Witness,
};
use musig2::KeyAggContext;
use strata_primitives::{bridge::PublickeyTable, constants::UNSPENDABLE_PUBLIC_KEY};

use super::{constants::MAGIC_BYTES, errors::BridgeTxBuilderError};
use crate::errors::BridgeTxBuilderResult;

/// Create a script with the spending condition that a MuSig2 aggregated signature corresponding to
/// the pubkey set must be provided.
pub fn n_of_n_script(aggregated_pubkey: &XOnlyPublicKey) -> ScriptBuf {
    Builder::new()
        .push_x_only_key(aggregated_pubkey)
        .push_opcode(OP_CHECKSIG)
        .into_script()
}

/// Aggregate the pubkeys using [`musig2`] and return the resulting [`XOnlyPublicKey`].
///
/// Please refer to MuSig2 key aggregation section in
/// [BIP 327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki).
pub fn get_aggregated_pubkey(pubkeys: PublickeyTable) -> XOnlyPublicKey {
    let key_agg_ctx =
        KeyAggContext::new(pubkeys.0.values().copied()).expect("key aggregation of musig2 pubkeys");

    let aggregated_pubkey: PublicKey = key_agg_ctx.aggregated_pubkey();

    aggregated_pubkey.x_only_public_key().0
}

/// Create the metadata script that "stores" the execution layer address information.
pub fn metadata_script(el_address: &[u8; 20]) -> ScriptBuf {
    let mut data = PushBytesBuf::new();
    data.extend_from_slice(MAGIC_BYTES)
        .expect("MAGIC_BYTES should be within the limit");
    data.extend_from_slice(&el_address[..])
        .expect("el_address should be within the limit");

    Builder::new()
        .push_opcode(OP_RETURN)
        .push_slice(data)
        .into_script()
}

/// Different spending paths for a taproot.
///
/// It can be a key path spend, a script path spend or both.
#[derive(Debug, Clone)]
pub enum SpendPath<'path> {
    /// Key path spend that requires just an untweaked (internal) public key.
    KeySpend {
        /// The internal key used to construct the taproot.
        internal_key: UntweakedPublicKey,
    },
    /// Script path spend that only allows spending via scripts in the taproot tree, with the
    /// internal key being the [`static@UNSPENDABLE_PUBLIC_KEY`].
    ScriptSpend {
        /// The scripts that live in the leaves of the taproot tree.
        scripts: &'path [ScriptBuf],
    },
    /// Allows spending via either a provided internal key or via scripts in the taproot tree.
    Both {
        /// The internal key used to construct the taproot.
        internal_key: UntweakedPublicKey,

        /// The scripts that live in the leaves of the taproot tree.
        scripts: &'path [ScriptBuf],
    },
}

/// Create a taproot address for the given `scripts` and `internal_key`.
///
/// # Errors
///
/// If the scripts is empty in [`SpendPath::ScriptSpend`].
pub fn create_taproot_addr<'creator>(
    network: &'creator Network,
    spend_path: SpendPath<'creator>,
) -> Result<(Address, TaprootSpendInfo), BridgeTxBuilderError> {
    match spend_path {
        SpendPath::KeySpend { internal_key } => build_taptree(internal_key, *network, &[]),
        SpendPath::ScriptSpend { scripts } => {
            if scripts.is_empty() {
                return Err(BridgeTxBuilderError::EmptyTapscript);
            }

            build_taptree(*UNSPENDABLE_PUBLIC_KEY, *network, scripts)
        }
        SpendPath::Both {
            internal_key,
            scripts,
        } => build_taptree(internal_key, *network, scripts),
    }
}

/// Constructs the taptree for the given scripts.
///
/// A taptree is a merkle tree made up of various scripts. Each script is a leaf in the merkle tree.
/// If the number of scripts is a power of 2, all the scripts lie at the deepest level (depth = n)
/// in the tree. If the number is not a power of 2, there are some scripts that will exist at the
/// penultimate level (depth = n - 1).
///
/// This function adds the scripts to the taptree after it computes the depth for each script.
fn build_taptree(
    internal_key: UntweakedPublicKey,
    network: Network,
    scripts: &[ScriptBuf],
) -> BridgeTxBuilderResult<(Address, TaprootSpendInfo)> {
    let mut taproot_builder = TaprootBuilder::new();

    let num_scripts = scripts.len();

    // Compute the height of the taptree required to fit in all the scripts.
    // If the script count <= 1, the depth should be 0. Otherwise, we compute the log. For example,
    // 2 scripts can fit in a height of 1 (0 being the root node). 4 can fit in a height of 2 and so
    // on.
    let max_depth = if num_scripts > 1 {
        (num_scripts - 1).ilog2() + 1
    } else {
        0
    };

    // Compute the maximum number of scripts that can fit in the taproot. For example, at a depth of
    // 3, we can fit 8 scripts.
    //              [Root Hash]
    //              /          \
    //             /            \
    //        [Hash 0]           [Hash 1]
    //       /        \          /      \
    //      /          \        /        \
    // [Hash 00]   [Hash 01] [Hash 10] [Hash 11]
    //   /   \       /   \     /   \     /   \
    // S0    S1    S2    S3  S4    S5   S6    S7
    let max_num_scripts = 2usize.pow(max_depth);

    // But we may be given say 5 scripts, in which case the tree would not be fully complete and we
    // need to add leaves at a shallower point in a way that minimizes the overall height (to reduce
    // the size of the merkle proof). So, we need to compute how many such scripts exist and add
    // these, at the appropriate depth.
    //
    //              [Root Hash]
    //              /          \
    //             /            \
    //        [Hash 0]          [Hash 1]
    //       /        \          /    \
    //      /          \        /      \
    // [Hash 00]        S2    S4        S5  ---> penultimate depth has 3 scripts
    //   /   \
    // S0    S1   ---------> max depth has 2 scripts
    let num_penultimate_scripts = max_num_scripts.saturating_sub(num_scripts);
    let num_deepest_scripts = num_scripts.saturating_sub(num_penultimate_scripts);

    for (script_idx, script) in scripts.iter().enumerate() {
        let depth = if script_idx < num_deepest_scripts {
            max_depth as u8
        } else {
            // if the deepest node is not filled, use the node at the upper level instead
            (max_depth - 1) as u8
        };

        taproot_builder = taproot_builder.add_leaf(depth, script.clone())?;
    }

    let spend_info = taproot_builder.finalize(SECP256K1, internal_key)?;
    let merkle_root = spend_info.merkle_root();

    Ok((
        Address::p2tr(SECP256K1, internal_key, merkle_root, network),
        spend_info,
    ))
}

/// Create an output that can be spent by anyone, i.e. its script contains a single `OP_TRUE`.
pub fn anyone_can_spend_txout() -> TxOut {
    let script = Builder::new().push_opcode(OP_TRUE).into_script();
    let script_pubkey = script.to_p2wsh();
    let value = script_pubkey.minimal_non_dust();

    TxOut {
        script_pubkey,
        value,
    }
}

/// Create a bitcoin [`Transaction`] for the given inputs and outputs.
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
/// This wraps the [`OutPoint`] in a structure that includes an empty `witness`, an empty
/// `script_sig` and the `sequence` set to enable replace-by-fee with no locktime.
pub fn create_tx_ins(utxos: impl IntoIterator<Item = OutPoint>) -> Vec<TxIn> {
    let mut tx_ins = Vec::new();

    for utxo in utxos {
        tx_ins.push(TxIn {
            previous_output: utxo,
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
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
    scripts_and_amounts
        .into_iter()
        .map(|(script_pubkey, value)| TxOut {
            script_pubkey,
            value,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use bitcoin::{key::Keypair, secp256k1::SecretKey};
    use rand_core::OsRng;

    use super::*;

    #[test]
    fn test_create_taproot_addr() {
        // create a bunch of dummy scripts to add to the taptree
        let max_scripts = 10;
        let scripts: Vec<ScriptBuf> = vec![ScriptBuf::from_bytes(vec![2u8; 32]); max_scripts];

        let network = Network::Regtest;

        let spend_path = SpendPath::ScriptSpend {
            scripts: &scripts[0..1],
        };
        assert!(
            create_taproot_addr(&network, spend_path).is_ok(),
            "should work if the number of scripts is exactly 1 i.e., only root node exists"
        );

        let spend_path = SpendPath::ScriptSpend {
            scripts: &scripts[0..4],
        };
        assert!(
            create_taproot_addr(&network, spend_path).is_ok(),
            "should work if the number of scripts is an exact power of 2"
        );

        let spend_path = SpendPath::ScriptSpend {
            scripts: &scripts[..],
        };
        assert!(
            create_taproot_addr(&network, spend_path).is_ok(),
            "should work if the number of scripts is not an exact power of 2"
        );

        let secret_key = SecretKey::new(&mut OsRng);
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_key);
        let (x_only_public_key, _) = XOnlyPublicKey::from_keypair(&keypair);

        let spend_path = SpendPath::KeySpend {
            internal_key: x_only_public_key,
        };
        assert!(
            create_taproot_addr(&network, spend_path).is_ok(),
            "should support empty scripts with some internal key"
        );

        let spend_path = SpendPath::Both {
            internal_key: x_only_public_key,
            scripts: &scripts[..3],
        };
        assert!(
            create_taproot_addr(&network, spend_path).is_ok(),
            "should support scripts with some internal key"
        );
    }
}
