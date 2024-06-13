use core::{result::Result::Ok, str::FromStr};

use anyhow::anyhow;
use bitcoin::absolute::LockTime;
use bitcoin::consensus::deserialize;
use bitcoin::key::UntweakedKeypair;
use bitcoin::sighash::Prevouts;
use bitcoin::Amount;
use bitcoin::{
    blockdata::{
        opcodes::{
            all::{OP_CHECKSIG, OP_ENDIF, OP_IF},
            OP_FALSE,
        },
        script,
    },
    hashes::{sha256d, Hash},
    key::{TapTweak, TweakedPublicKey},
    script::PushBytesBuf,
    secp256k1::{
        self, constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature, Secp256k1, SecretKey,
        XOnlyPublicKey,
    },
    sighash::SighashCache,
    taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder},
    transaction::Version,
    Address, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use rand::RngCore;

use crate::rpc::types::RawUTXO;

use super::L1WriteIntent;

const BITCOIN_DUST_LIMIT: u64 = 546;

const BATCH_DATA_TAG: &[u8] = &[1];
const PROOF_DATA_TAG: &[u8] = &[2];
const PUBLICKEY_TAG: &[u8] = &[3];
const ROLLUP_NAME_TAG: &[u8] = &[6];
const SIGNATURE_TAG: &[u8] = &[7];

#[derive(Clone, Debug, PartialEq)]
pub struct UTXO {
    pub txid: Txid,
    pub vout: u32,
    pub address: String,
    pub script_pubkey: String,
    pub amount: u64,
    pub confirmations: u64,
    pub spendable: bool,
    pub solvable: bool,
}

#[derive(Debug)]
pub enum UtxoParseError {
    InvalidHexString,
    InvalidTxId,
}

impl TryFrom<RawUTXO> for UTXO {
    type Error = UtxoParseError;
    fn try_from(value: RawUTXO) -> Result<Self, Self::Error> {
        let rawtxid = value.txid;
        let txid =
            deserialize(&hex::decode(rawtxid).map_err(|_| UtxoParseError::InvalidHexString)?)
                .map_err(|_| UtxoParseError::InvalidTxId)?;
        Ok(UTXO {
            txid,
            vout: value.vout,
            address: value.address,
            script_pubkey: value.script_pub_key,
            amount: (value.amount * 100_000_000.0) as u64,
            confirmations: value.confirmations,
            spendable: value.spendable,
            solvable: value.solvable,
        })
    }
}

// Signs a message with a private key
pub fn sign_blob_with_private_key(
    blob: &[u8],
    private_key: &SecretKey,
) -> Result<(Vec<u8>, Vec<u8>), ()> {
    let message = sha256d::Hash::hash(blob).to_byte_array();
    let secp = Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, private_key);
    let msg = secp256k1::Message::from_slice(&message).unwrap();
    let sig = secp.sign_ecdsa(&msg, private_key);
    Ok((
        sig.serialize_compact().to_vec(),
        public_key.serialize().to_vec(),
    ))
}

fn get_size(
    inputs: &[TxIn],
    outputs: &[TxOut],
    script: Option<&ScriptBuf>,
    control_block: Option<&ControlBlock>,
) -> usize {
    let mut tx = Transaction {
        input: inputs.to_vec(),
        output: outputs.to_vec(),
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    for i in 0..tx.input.len() {
        tx.input[i].witness.push(
            Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
                .unwrap()
                .as_ref(),
        );
    }

    #[allow(clippy::unnecessary_unwrap)]
    if tx.input.len() == 1 && script.is_some() && control_block.is_some() {
        tx.input[0].witness.push(script.unwrap());
        tx.input[0].witness.push(control_block.unwrap().serialize());
    }

    tx.vsize()
}

fn choose_utxos(utxos: &[UTXO], amount: u64) -> Result<(Vec<UTXO>, u64), anyhow::Error> {
    let mut bigger_utxos: Vec<&UTXO> = utxos.iter().filter(|utxo| utxo.amount >= amount).collect();
    let mut sum: u64 = 0;

    if !bigger_utxos.is_empty() {
        // sort vec by amount (small first)
        bigger_utxos.sort_by(|a, b| a.amount.cmp(&b.amount));

        // single utxo will be enough
        // so return the transaction
        let utxo = bigger_utxos[0];
        sum += utxo.amount;

        Ok((vec![utxo.clone()], sum))
    } else {
        let mut smaller_utxos: Vec<&UTXO> =
            utxos.iter().filter(|utxo| utxo.amount < amount).collect();

        // sort vec by amount (large first)
        smaller_utxos.sort_by(|a, b| b.amount.cmp(&a.amount));

        let mut chosen_utxos: Vec<UTXO> = vec![];

        for utxo in smaller_utxos {
            sum += utxo.amount;
            chosen_utxos.push(utxo.clone());

            if sum >= amount {
                break;
            }
        }

        if sum < amount {
            return Err(anyhow!("not enough UTXOs"));
        }

        Ok((chosen_utxos, sum))
    }
}

fn build_commit_transaction(
    rest_utxos: Vec<UTXO>,
    recipient: Address,
    change_address: Address,
    output_value: u64,
    fee_rate: f64,
) -> Result<(Transaction, Vec<UTXO>), anyhow::Error> {
    // get single input single output transaction size
    let mut size = get_size(
        &[TxIn {
            previous_output: OutPoint {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
                vout: 0,
            },
            script_sig: script::Builder::new().into_script(),
            witness: Witness::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        }],
        &[TxOut {
            script_pubkey: recipient.clone().script_pubkey(),
            value: Amount::from_sat(output_value),
        }],
        None,
        None,
    );
    let mut last_size = size;

    let utxos: Vec<UTXO> = rest_utxos
        .iter()
        .filter(|utxo| utxo.spendable && utxo.solvable && utxo.amount > BITCOIN_DUST_LIMIT)
        .cloned()
        .collect();

    if utxos.is_empty() {
        return Err(anyhow::anyhow!(format!(
            "no spendable UTXOs greater than dust ({})",
            BITCOIN_DUST_LIMIT
        )));
    }

    let (commit_txn, consumed_utxo) = loop {
        let fee = ((last_size as f64) * fee_rate).ceil() as u64;

        let input_total = output_value + fee;

        let res = choose_utxos(&utxos, input_total)?;

        let (mut chosen_utxos, mut sum) = res;

        let mut outputs: Vec<TxOut> = vec![];
        outputs.push(TxOut {
            value: Amount::from_sat(output_value),
            script_pubkey: recipient.script_pubkey(),
        });

        let mut direct_return = false;
        if let Some(excess) = sum.checked_sub(input_total) {
            if excess >= BITCOIN_DUST_LIMIT {
                outputs.push(TxOut {
                    value: Amount::from_sat(excess),
                    script_pubkey: change_address.script_pubkey(),
                });
            } else {
                // if dust is left, leave it for fee
                direct_return = true;
            }
        }

        let inputs: Vec<TxIn> = chosen_utxos
            .iter()
            .map(|u| TxIn {
                previous_output: OutPoint {
                    txid: u.txid,
                    vout: u.vout,
                },
                script_sig: script::Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect();

        size = get_size(&inputs, &outputs, None, None);

        if size == last_size || direct_return {
            let commit_txn = Transaction {
                lock_time: LockTime::ZERO,
                version: Version(2),
                input: inputs,
                output: outputs,
            };

            break (commit_txn, chosen_utxos);
        }

        last_size = size;
    };

    Ok((commit_txn, consumed_utxo))
}

fn build_reveal_transaction(
    input_transaction: Transaction,
    recipient: Address,
    output_value: u64,
    fee_rate: f64,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
) -> Result<(Transaction, Vec<UTXO>), anyhow::Error> {
    let outputs: Vec<TxOut> = vec![TxOut {
        value: Amount::from_sat(output_value),
        script_pubkey: recipient.script_pubkey(),
    }];

    let v_out_for_reveal = 0u32;
    let input_utxo = input_transaction.output[v_out_for_reveal as usize].clone();
    let txn_id = input_transaction.compute_txid();

    let inputs = vec![TxIn {
        previous_output: OutPoint {
            txid: txn_id,
            vout: v_out_for_reveal,
        },
        script_sig: script::Builder::new().into_script(),
        witness: Witness::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
    }];
    let size = get_size(&inputs, &outputs, Some(reveal_script), Some(control_block));
    let fee = ((size as f64) * fee_rate).ceil() as u64;
    let input_total = Amount::from_sat(output_value + fee);
    if input_utxo.value < Amount::from_sat(BITCOIN_DUST_LIMIT) || input_utxo.value < input_total {
        return Err(anyhow::anyhow!("input UTXO not big enough"));
    }
    let tx = Transaction {
        lock_time: LockTime::ZERO,
        version: Version(2),
        input: inputs,
        output: outputs,
    };

    // make the produced UTXO from reveal txn
    let mut reveal_produced_utxos: Vec<UTXO> = tx
        .output
        .iter()
        .enumerate()
        .map(|(index, el)| UTXO {
            txid: tx.compute_txid(),
            vout: index as u32,
            address: recipient.to_string(),
            script_pubkey: recipient.script_pubkey().to_hex_string(),
            amount: el.value.to_sat(),
            confirmations: 0,
            spendable: true,
            solvable: true,
        })
        .collect();

    // make the produced UTXO from the commit txn
    // output index from `1` is taken as `0` is consumed by reveal txn
    let mut commit_produced_utxos: Vec<UTXO> = input_transaction.output[1..]
        .iter()
        .enumerate()
        .map(|(index, el)| UTXO {
            txid: input_transaction.compute_txid(),
            vout: (index + 1) as u32,
            address: recipient.to_string(),
            script_pubkey: recipient.script_pubkey().to_hex_string(),
            amount: el.value.to_sat(),
            confirmations: 0,
            spendable: true,
            solvable: true,
        })
        .collect();

    // combine all produced utxos
    commit_produced_utxos.append(&mut reveal_produced_utxos);

    Ok((tx, commit_produced_utxos))
}

// TODO: parametrize hardness
// so tests are easier
// Creates the inscription transactions (commit and reveal)
#[allow(clippy::too_many_arguments)]
pub fn create_inscription_transactions(
    rollup_name: &str,
    write_intent: L1WriteIntent,
    sequencer_public_key: Vec<u8>,
    utxos: Vec<UTXO>,
    recipient: Address,
    reveal_value: u64,
    fee_rate: f64,
    network: Network,
) -> Result<(Transaction, Transaction), anyhow::Error> {
    // Create commit key
    let secp256k1 = Secp256k1::new();

    let mut rand_bytes = [0; 32];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &mut rand_bytes)?;
    let (public_key, _parity) = XOnlyPublicKey::from_keypair(&key_pair);

    // start creating inscription content
    let reveal_script_builder = script::Builder::new()
        .push_x_only_key(&public_key)
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(PushBytesBuf::try_from(ROLLUP_NAME_TAG.to_vec()).expect("Cannot push tag"))
        .push_slice(
            PushBytesBuf::try_from(rollup_name.as_bytes().to_vec())
                .expect("Cannot push rollup name"),
        )
        .push_slice(
            PushBytesBuf::try_from(SIGNATURE_TAG.to_vec()).expect("Cannot push signature tag"),
        )
        .push_slice(
            PushBytesBuf::try_from(write_intent.batch_signature).expect("Cannot push signature"),
        )
        .push_slice(
            PushBytesBuf::try_from(PUBLICKEY_TAG.to_vec()).expect("Cannot push public key tag"),
        )
        .push_slice(
            PushBytesBuf::try_from(sequencer_public_key).expect("Cannot push sequencer public key"),
        );

    let mut reveal_script_builder = reveal_script_builder.clone();
    // PUSH batch and proofs. First push tag and their corresponding size and then chunks of body
    reveal_script_builder = reveal_script_builder
        .push_slice(
            PushBytesBuf::try_from(BATCH_DATA_TAG.to_vec()).expect("Cannot push batch data tag"),
        )
        // Push batch size
        .push_int(write_intent.batch_data.len() as i64);

    // push body in chunks of 520 bytes
    for chunk in write_intent.batch_data.chunks(520) {
        reveal_script_builder = reveal_script_builder
            .push_slice(PushBytesBuf::try_from(chunk.to_vec()).expect("Cannot push body chunk"));
    }

    reveal_script_builder = reveal_script_builder
        .push_slice(
            PushBytesBuf::try_from(PROOF_DATA_TAG.to_vec()).expect("Cannot push proof data tag"),
        )
        // push proof size
        .push_int(write_intent.proof_data.len() as i64);

    // push body in chunks of 520 bytes
    for chunk in write_intent.proof_data.chunks(520) {
        reveal_script_builder = reveal_script_builder
            .push_slice(PushBytesBuf::try_from(chunk.to_vec()).expect("Cannot push body chunk"));
    }

    // push end if
    reveal_script_builder = reveal_script_builder.push_opcode(OP_ENDIF);

    // finalize reveal script
    let reveal_script = reveal_script_builder.into_script();

    // create spend info for tapscript
    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, reveal_script.clone())
        .expect("Cannot add reveal script to taptree")
        .finalize(&secp256k1, public_key)
        .expect("Cannot finalize taptree");

    // create control block for tapscript
    let control_block = taproot_spend_info
        .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
        .expect("Cannot create control block");

    // create commit tx address
    let commit_tx_address = Address::p2tr(
        &secp256k1,
        public_key,
        taproot_spend_info.merkle_root(),
        network,
    );

    let commit_value = (get_size(
        &[TxIn {
            previous_output: OutPoint {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
                vout: 0,
            },
            script_sig: script::Builder::new().into_script(),
            witness: Witness::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        }],
        &[TxOut {
            script_pubkey: recipient.clone().script_pubkey(),
            value: Amount::from_sat(reveal_value),
        }],
        Some(&reveal_script),
        Some(&control_block),
    ) as f64
        * fee_rate
        + reveal_value as f64)
        .ceil() as u64;

    // build commit tx
    let (unsigned_commit_tx, consumed_utxos) = build_commit_transaction(
        utxos,
        commit_tx_address.clone(),
        recipient.clone(),
        commit_value,
        fee_rate,
    )?;

    let output_to_reveal = unsigned_commit_tx.output[0].clone();

    let (mut reveal_tx, produced_utxos) = build_reveal_transaction(
        unsigned_commit_tx.clone(),
        recipient,
        reveal_value,
        fee_rate,
        &reveal_script,
        &control_block,
    )?;

    // start signing reveal tx
    let mut sighash_cache = SighashCache::new(&mut reveal_tx);

    // create data to sign
    let signature_hash = sighash_cache
        .taproot_script_spend_signature_hash(
            0,
            &Prevouts::All(&[output_to_reveal]),
            TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript),
            bitcoin::sighash::TapSighashType::Default,
        )
        .expect("Cannot create hash for signature");

    // sign reveal tx data
    let mut randbytes = [0; 32];
    rand::thread_rng().fill_bytes(&mut randbytes);

    let signature = secp256k1.sign_schnorr_with_aux_rand(
        &secp256k1::Message::from_digest_slice(signature_hash.as_byte_array())
            .expect("should be cryptographically secure hash"),
        &key_pair,
        &randbytes,
    );

    // add signature to witness and finalize reveal tx
    let witness = sighash_cache.witness_mut(0).unwrap();
    witness.push(signature.as_ref());
    witness.push(reveal_script);
    witness.push(&control_block.serialize());

    // check if inscription locked to the correct address
    let recovery_key_pair = key_pair.tap_tweak(&secp256k1, taproot_spend_info.merkle_root());
    let (x_only_pub_key, _parity) = recovery_key_pair.to_inner().x_only_public_key();
    assert_eq!(
        Address::p2tr_tweaked(
            TweakedPublicKey::dangerous_assume_tweaked(x_only_pub_key),
            network,
        ),
        commit_tx_address
    );

    return Ok((unsigned_commit_tx, reveal_tx)); // , utxo_change_log));
}

// #[cfg(test)]
// mod tests {
//     use core::str::FromStr;
//
// use brotli::CompressorWriter;
// use brotli::DecompressorWriter;
//     use bitcoin::{
//         absolute::LockTime,
//         hashes::Hash,
//         script,
//         secp256k1::{constants::SCHNORR_SIGNATURE_SIZE, schnorr::Signature},
//         taproot::ControlBlock,
//         Address, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
//     };
//
//     use crate::{
//         helpers::{
//             builders::{compress_blob, decompress_blob},
//             parsers::parse_transaction,
//             REVEAL_TX_HASH_PREFIX,
//         },
//         spec::utxo::UTXO,
//         REVEAL_OUTPUT_AMOUNT,
//     };
//     pub fn compress_blob(blob: &[u8]) -> Vec<u8> {
//         let mut writer = CompressorWriter::new(Vec::new(), 4096, 11, 22);
//         writer.write_all(blob).unwrap();
//         writer.into_inner()
//     }
//
//     pub fn decompress_blob(blob: &[u8]) -> Vec<u8> {
//         let mut writer = DecompressorWriter::new(Vec::new(), 4096);
//         writer.write_all(blob).unwrap();
//         writer.into_inner().expect("decompression failed")
//     }
//

//     #[test]
//     fn compression_decompression() {
//         let blob = std::fs::read("test_data/blob.txt").unwrap();
//
//         // compress and measure time
//         let time = std::time::Instant::now();
//         let compressed_blob = compress_blob(&blob);
//         println!("compression time: {:?}", time.elapsed());
//
//         // decompress and measure time
//         let time = std::time::Instant::now();
//         let decompressed_blob = decompress_blob(&compressed_blob);
//         println!("decompression time: {:?}", time.elapsed());
//
//         assert_eq!(blob, decompressed_blob);
//
//         // size
//         println!("blob size: {}", blob.len());
//         println!("compressed blob size: {}", compressed_blob.len());
//         println!(
//             "compression ratio: {}",
//             (blob.len() as f64) / (compressed_blob.len() as f64)
//         );
//     }
//
//     #[allow(clippy::type_complexity)]
//     fn get_mock_data() -> (
//         &'static str,
//         Vec<u8>,
//         Vec<u8>,
//         Vec<u8>,
//         Address,
//         Option<UTXO>,
//         Vec<UTXO>,
//     ) {
//         let rollup_name = "test_rollup";
//         let body = vec![100; 1000];
//         let signature = vec![100; 64];
//         let sequencer_public_key = vec![100; 33];
//         let address =
//             Address::from_str("bc1pp8qru0ve43rw9xffmdd8pvveths3cx6a5t6mcr0xfn9cpxx2k24qf70xq9")
//                 .unwrap()
//                 .require_network(bitcoin::Network::Bitcoin)
//                 .unwrap();
//
//         let first_utxo = Some(UTXO {
//             txid: Txid::from_str(
//                 "4cfbec13cf1510545f285cceceb6229bd7b6a918a8f6eba1dbee64d26226a3b7",
//             )
//             .unwrap(),
//             vout: 0,
//             address: "bc1pp8qru0ve43rw9xffmdd8pvveths3cx6a5t6mcr0xfn9cpxx2k24qf70xq9".to_string(),
//             script_pubkey: address.script_pubkey().to_hex_string(),
//             amount: 1_000_000,
//             confirmations: 100,
//             spendable: true,
//             solvable: true,
//         });
//
//         let rest_utxos = vec![
//             UTXO {
//                 txid: Txid::from_str(
//                     "44990141674ff56ed6fee38879e497b2a726cddefd5e4d9b7bf1c4e561de4347",
//                 )
//                 .unwrap(),
//                 vout: 0,
//                 address: "bc1pp8qru0ve43rw9xffmdd8pvveths3cx6a5t6mcr0xfn9cpxx2k24qf70xq9"
//                     .to_string(),
//                 script_pubkey: address.script_pubkey().to_hex_string(),
//                 amount: 100_000,
//                 confirmations: 100,
//                 spendable: true,
//                 solvable: true,
//             },
//             UTXO {
//                 txid: Txid::from_str(
//                     "4dbe3c10ee0d6bf16f9417c68b81e963b5bccef3924bbcb0885c9ea841912325",
//                 )
//                 .unwrap(),
//                 vout: 0,
//                 address: "bc1pp8qru0ve43rw9xffmdd8pvveths3cx6a5t6mcr0xfn9cpxx2k24qf70xq9"
//                     .to_string(),
//                 script_pubkey: address.script_pubkey().to_hex_string(),
//                 amount: 10_000,
//                 confirmations: 100,
//                 spendable: true,
//                 solvable: true,
//             },
//         ];
//
//         (
//             rollup_name,
//             body,
//             signature,
//             sequencer_public_key,
//             address,
//             first_utxo,
//             rest_utxos,
//         )
//     }
//
//     #[test]
//     fn choose_utxos() {
//         let (_, _, _, _, _, first_utxo, mut rest_utxos) = get_mock_data();
//         let mut utxos = Vec::with_capacity(rest_utxos.len() + 1);
//         utxos.push(first_utxo.unwrap());
//         utxos.append(&mut rest_utxos);
//
//         let (chosen_utxos, sum) = super::choose_utxos(&utxos, 105_000).unwrap();
//
//         assert_eq!(sum, 1_000_000);
//         assert_eq!(chosen_utxos.len(), 1);
//         assert_eq!(chosen_utxos[0], utxos[0]);
//
//         let (chosen_utxos, sum) = super::choose_utxos(&utxos, 1_005_000).unwrap();
//
//         assert_eq!(sum, 1_100_000);
//         assert_eq!(chosen_utxos.len(), 2);
//         assert_eq!(chosen_utxos[0], utxos[0]);
//         assert_eq!(chosen_utxos[1], utxos[1]);
//
//         let (chosen_utxos, sum) = super::choose_utxos(&utxos, 100_000).unwrap();
//
//         assert_eq!(sum, 100_000);
//         assert_eq!(chosen_utxos.len(), 1);
//         assert_eq!(chosen_utxos[0], utxos[1]);
//
//         let (chosen_utxos, sum) = super::choose_utxos(&utxos, 90_000).unwrap();
//
//         assert_eq!(sum, 100_000);
//         assert_eq!(chosen_utxos.len(), 1);
//         assert_eq!(chosen_utxos[0], utxos[1]);
//
//         let res = super::choose_utxos(&utxos, 100_000_000);
//
//         assert!(res.is_err());
//         assert_eq!(format!("{}", res.unwrap_err()), "not enough UTXOs");
//     }
//
//     #[test]
//     #[ignore = "fixme"]
//     fn build_commit_transaction() {
//         let (_, _, _, _, address, first_utxo, rest_utxos) = get_mock_data();
//
//         let recipient =
//             Address::from_str("bc1p2e37kuhnsdc5zvc8zlj2hn6awv3ruavak6ayc8jvpyvus59j3mwqwdt0zc")
//                 .unwrap()
//                 .require_network(bitcoin::Network::Bitcoin)
//                 .unwrap();
//         let (mut tx, _) = super::build_commit_transaction(
//             first_utxo.clone(),
//             rest_utxos.clone(),
//             recipient.clone(),
//             address.clone(),
//             5_000,
//             8.0,
//         )
//         .unwrap();
//
//         tx.input[0].witness.push(
//             Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
//                 .unwrap()
//                 .as_ref(),
//         );
//
//         // 154 vB * 8 sat/vB = 1232 sats
//         // 5_000 + 1232 = 6232
//         // input: 10000
//         // outputs: 5_000 + 3.768
//         assert_eq!(tx.vsize(), 154);
//         assert_eq!(tx.input.len(), 1);
//         assert_eq!(tx.output.len(), 2);
//         assert_eq!(tx.output[0].value, 5_000);
//         assert_eq!(tx.output[0].script_pubkey, recipient.script_pubkey());
//         assert_eq!(tx.output[1].value, 3_768);
//         assert_eq!(tx.output[1].script_pubkey, address.script_pubkey());
//
//         let (mut tx, _) = super::build_commit_transaction(
//             first_utxo.clone(),
//             rest_utxos.clone(),
//             recipient.clone(),
//             address.clone(),
//             5_000,
//             45.0,
//         )
//         .unwrap();
//
//         tx.input[0].witness.push(
//             Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
//                 .unwrap()
//                 .as_ref(),
//         );
//
//         // 111 vB * 45 sat/vB = 4.995 sats
//         // 5_000 + 4928 = 9995
//         // input: 10000
//         // outputs: 5_000
//         assert_eq!(tx.vsize(), 111);
//         assert_eq!(tx.input.len(), 1);
//         assert_eq!(tx.output.len(), 1);
//         assert_eq!(tx.output[0].value, 5_000);
//         assert_eq!(tx.output[0].script_pubkey, recipient.script_pubkey());
//
//         let (mut tx, _) = super::build_commit_transaction(
//             first_utxo.clone(),
//             rest_utxos.clone(),
//             recipient.clone(),
//             address.clone(),
//             5_000,
//             32.0,
//         )
//         .unwrap();
//
//         tx.input[0].witness.push(
//             Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
//                 .unwrap()
//                 .as_ref(),
//         );
//
//         // you expect
//         // 154 vB * 32 sat/vB = 4.928 sats
//         // 5_000 + 4928 = 9928
//         // input: 10000
//         // outputs: 5_000 72
//         // instead do
//         // input: 10000
//         // outputs: 5_000
//         // so size is actually 111
//         assert_eq!(tx.vsize(), 111);
//         assert_eq!(tx.input.len(), 1);
//         assert_eq!(tx.output.len(), 1);
//         assert_eq!(tx.output[0].value, 5_000);
//         assert_eq!(tx.output[0].script_pubkey, recipient.script_pubkey());
//
//         let (mut tx, _) = super::build_commit_transaction(
//             first_utxo.clone(),
//             rest_utxos.clone(),
//             recipient.clone(),
//             address.clone(),
//             1_050_000,
//             5.0,
//         )
//         .unwrap();
//
//         tx.input[0].witness.push(
//             Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
//                 .unwrap()
//                 .as_ref(),
//         );
//         tx.input[1].witness.push(
//             Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
//                 .unwrap()
//                 .as_ref(),
//         );
//
//         // 212 vB * 5 sat/vB = 1060 sats
//         // 1_050_000 + 1060 = 1_051_060
//         // inputs: 1_000_000 100_000
//         // outputs: 1_050_000 8940
//         assert_eq!(tx.vsize(), 212);
//         assert_eq!(tx.input.len(), 2);
//         assert_eq!(tx.output.len(), 2);
//         assert_eq!(tx.output[0].value, 1_050_000);
//         assert_eq!(tx.output[0].script_pubkey, recipient.script_pubkey());
//         assert_eq!(tx.output[1].value, 48940);
//         assert_eq!(tx.output[1].script_pubkey, address.script_pubkey());
//
//         let tx = super::build_commit_transaction(
//             first_utxo.clone(),
//             rest_utxos.clone(),
//             recipient.clone(),
//             address.clone(),
//             100_000_000_000,
//             32.0,
//         );
//
//         assert!(tx.is_err());
//         assert_eq!(format!("{}", tx.unwrap_err()), "not enough UTXOs");
//
//         let tx = super::build_commit_transaction(
//             None,
//             vec![UTXO {
//                 txid: Txid::from_str(
//                     "4cfbec13cf1510545f285cceceb6229bd7b6a918a8f6eba1dbee64d26226a3b7",
//                 )
//                 .unwrap(),
//                 vout: 0,
//                 address: "bc1pp8qru0ve43rw9xffmdd8pvveths3cx6a5t6mcr0xfn9cpxx2k24qf70xq9"
//                     .to_string(),
//                 script_pubkey: address.script_pubkey().to_hex_string(),
//                 amount: 152,
//                 confirmations: 100,
//                 spendable: true,
//                 solvable: true,
//             }],
//             recipient.clone(),
//             address.clone(),
//             100_000_000_000,
//             32.0,
//         );
//
//         assert!(tx.is_err());
//         assert_eq!(format!("{}", tx.unwrap_err()), "no spendable UTXOs");
//     }
//
//     fn get_txn_from_utxo(utxo: &UTXO, _address: &Address) -> Transaction {
//         let inputs = vec![TxIn {
//             previous_output: OutPoint {
//                 txid: utxo.txid,
//                 vout: utxo.vout,
//             },
//             script_sig: script::Builder::new().into_script(),
//             witness: Witness::new(),
//             sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
//         }];
//
//         let outputs = vec![TxOut {
//             value: utxo.amount,
//             script_pubkey: ScriptBuf::from_hex(utxo.script_pubkey.as_str()).unwrap(),
//         }];
//
//         Transaction {
//             lock_time: LockTime::ZERO,
//             version: bitcoin::transaction::Version(2),
//             input: inputs,
//             output: outputs,
//         }
//     }
//
//     #[test]
//     #[ignore = "fixme"]
//     fn build_reveal_transaction() {
//         let (_, _, _, _, address, _, rest_utxos) = get_mock_data();
//
//         let utxo = rest_utxos.first().unwrap();
//         let _script = ScriptBuf::from_hex("62a58f2674fd840b6144bea2e63ebd35c16d7fd40252a2f28b2a01a648df356343e47976d7906a0e688bf5e134b6fd21bd365c016b57b1ace85cf30bf1206e27").unwrap();
//         let control_block = ControlBlock::decode(&[
//             193, 165, 246, 250, 6, 222, 28, 9, 130, 28, 217, 67, 171, 11, 229, 62, 48, 206, 219,
//             111, 155, 208, 6, 7, 119, 63, 146, 90, 227, 254, 231, 232, 249,
//         ])
//         .unwrap(); // should be 33 bytes
//
//         let inp_txn = get_txn_from_utxo(utxo, &address);
//         let (mut tx, _) = super::build_reveal_transaction(
//             inp_txn,
//             address.clone(),
//             REVEAL_OUTPUT_AMOUNT,
//             8.0,
//             &_script,
//             &control_block,
//         )
//         .unwrap();
//
//         tx.input[0].witness.push([0; SCHNORR_SIGNATURE_SIZE]);
//         tx.input[0].witness.push(_script.clone());
//         tx.input[0].witness.push(control_block.serialize());
//
//         assert_eq!(tx.input.len(), 1);
//         assert_eq!(tx.input[0].previous_output.txid, utxo.txid);
//         assert_eq!(tx.input[0].previous_output.vout, utxo.vout);
//
//         assert_eq!(tx.output.len(), 1);
//         assert_eq!(tx.output[0].value, REVEAL_OUTPUT_AMOUNT);
//         assert_eq!(tx.output[0].script_pubkey, address.script_pubkey());
//
//         let utxo = rest_utxos.get(2).unwrap();
//         let inp_txn = get_txn_from_utxo(utxo, &address);
//         let tx = super::build_reveal_transaction(
//             inp_txn,
//             address.clone(),
//             REVEAL_OUTPUT_AMOUNT,
//             75.0,
//             &_script,
//             &control_block,
//         );
//
//         assert!(tx.is_err());
//         assert_eq!(format!("{}", tx.unwrap_err()), "input UTXO not big enough");
//
//         let utxo = rest_utxos.get(2).unwrap();
//         let inp_txn = get_txn_from_utxo(utxo, &address);
//         let tx = super::build_reveal_transaction(
//             inp_txn,
//             address.clone(),
//             9999,
//             1.0,
//             &_script,
//             &control_block,
//         );
//
//         assert!(tx.is_err());
//         assert_eq!(format!("{}", tx.unwrap_err()), "input UTXO not big enough");
//     }
//     #[test]
//     #[ignore = "fixme"]
//     fn create_inscription_transactions() {
//         let (rollup_name, body, signature, sequencer_public_key, address, _first_utxo, rest_utxos) =
//             get_mock_data();
//
//         let (first_utxo, rest_utxos) = rest_utxos.split_first().unwrap();
//
//         let (commit, reveal, _) = super::create_inscription_transactions(
//             rollup_name,
//             body.clone(),
//             signature.clone(),
//             sequencer_public_key.clone(),
//             Some(first_utxo.clone()),
//             rest_utxos.to_vec(),
//             address.clone(),
//             REVEAL_OUTPUT_AMOUNT,
//             12.0,
//             10.0,
//             bitcoin::Network::Bitcoin,
//         )
//         .unwrap();
//
//         // check pow
//         assert!(reveal
//             .txid()
//             .as_byte_array()
//             .starts_with(REVEAL_TX_HASH_PREFIX));
//
//         // check outputs
//         assert_eq!(commit.output.len(), 2, "commit tx should have 2 outputs");
//
//         assert_eq!(reveal.output.len(), 1, "reveal tx should have 1 output");
//
//         assert_eq!(
//             commit.input[0].previous_output.txid, rest_utxos[2].txid,
//             "utxo to inscribe should be chosen correctly"
//         );
//         assert_eq!(
//             commit.input[0].previous_output.vout, rest_utxos[2].vout,
//             "utxo to inscribe should be chosen correctly"
//         );
//
//         assert_eq!(
//             reveal.input[0].previous_output.txid,
//             commit.txid(),
//             "reveal should use commit as input"
//         );
//         assert_eq!(
//             reveal.input[0].previous_output.vout, 0,
//             "reveal should use commit as input"
//         );
//
//         assert_eq!(
//             reveal.output[0].script_pubkey,
//             address.script_pubkey(),
//             "reveal should pay to the correct address"
//         );
//
//         // check inscription
//         let inscription = parse_transaction(&reveal, rollup_name).unwrap();
//
//         assert_eq!(inscription.body, body, "body should be correct");
//         assert_eq!(
//             inscription.signature, signature,
//             "signature should be correct"
//         );
//         assert_eq!(
//             inscription.public_key, sequencer_public_key,
//             "sequencer public key should be correct"
//         );
//     }
// }
