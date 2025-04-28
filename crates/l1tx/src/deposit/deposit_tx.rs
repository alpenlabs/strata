//! parser types for Deposit Tx, and later deposit Request Tx

use bitcoin::{
    hashes::Hash,
    key::TapTweak,
    opcodes::all::OP_RETURN,
    sighash::{Prevouts, SighashCache},
    Amount, OutPoint, ScriptBuf, TapNodeHash, TapSighashType, Transaction, TxOut, XOnlyPublicKey,
};
use secp256k1::{schnorr::Signature, Message};
use strata_primitives::{
    buf::Buf32,
    l1::{DepositInfo, OutputRef, XOnlyPk},
    prelude::DepositTxParams,
};

use super::constants::*;
use crate::{
    deposit::error::DepositParseError,
    utils::{next_bytes, next_op},
};

const TAKEBACK_HASH_LEN: usize = 32;
const SATS_AMOUNT_LEN: usize = 8;
const DEPOSIT_IDX_LEN: usize = 4;

/// Extracts the DepositInfo from the Deposit Transaction
pub fn extract_deposit_info(tx: &Transaction, config: &DepositTxParams) -> Option<DepositInfo> {
    // Get the first output (index 0)
    let send_addr_out = tx.output.first()?;

    // Get the second output (index 1)
    let op_return_out = tx.output.get(1)?;

    // Check if it is exact BRIDGE_DENOMINATION amount
    // TODO make this not a const!
    if send_addr_out.value.to_sat() != BRIDGE_DENOMINATION.to_sat() {
        return None;
    }

    // Check if deposit output address matches.
    if send_addr_out.script_pubkey != config.address.address().script_pubkey() {
        return None;
    }

    // Parse the tag from the OP_RETURN output.
    let tag_data = parse_tag_script(&op_return_out.script_pubkey, config).ok()?;

    // Get the first input of the transaction
    let deposit_outpoint = OutputRef::from(OutPoint {
        txid: tx.compute_txid(),
        vout: 0, // deposit must always exist in the first output
    });

    // Check if it was signed off by the operators and hence verify that this is just not someone
    // else sending bitcoin to N-of-N address.
    validate_deposit_signature(tx, &tag_data, config)?;

    // Construct and return the DepositInfo
    Some(DepositInfo {
        deposit_idx: tag_data.deposit_idx,
        amt: send_addr_out.value.into(),
        address: tag_data.dest_buf.to_vec(),
        outpoint: deposit_outpoint,
    })
}

/// Validate that the transaction has been signed off by the N of N operators pubkey.
fn validate_deposit_signature(
    tx: &Transaction,
    tag_data: &DepositTag<'_>,
    dep_config: &DepositTxParams,
) -> Option<()> {
    // Initialize necessary variables and dependencies
    let secp = secp256k1::SECP256K1;

    // Extract and validate input signature
    let input = tx.input[0].clone();
    let sig_bytes = &input.witness[0];
    let schnorr_sig = Signature::from_slice(sig_bytes.get(..64)?).unwrap();

    // Parse the internal pubkey and merkle root
    let internal_pubkey = XOnlyPk::from_address(&dep_config.address).ok()?;
    let merkle_root: TapNodeHash = TapNodeHash::from_byte_array(*tag_data.tapscript_root.as_ref());

    let int_key = XOnlyPublicKey::from_slice(internal_pubkey.inner().as_bytes()).unwrap();

    // Build the scriptPubKey for the UTXO
    let script_pubkey = ScriptBuf::new_p2tr(secp, int_key, Some(merkle_root));

    let utxos = [TxOut {
        value: Amount::from_sat(tag_data.amount),
        script_pubkey,
    }];

    // Compute the sighash
    let prevout = Prevouts::All(&utxos);
    let sighash = SighashCache::new(tx)
        .taproot_key_spend_signature_hash(0, &prevout, TapSighashType::Default)
        .unwrap();

    // Prepare the message for signature verification
    let msg = Message::from_digest(*sighash.as_byte_array());

    // Compute the tweaked output key
    let (output_key, _) = int_key.tap_tweak(secp, Some(merkle_root));

    // Verify the Schnorr signature
    secp.verify_schnorr(&schnorr_sig, &msg, &output_key.to_inner())
        .ok()
}

struct DepositTag<'buf> {
    deposit_idx: u32,
    dest_buf: &'buf [u8],
    // TODO: better naming
    amount: u64,
    tapscript_root: Buf32,
}

/// extracts the EE address given that the script is OP_RETURN type and contains the Magic Bytes
fn parse_tag_script<'a>(
    script: &'a ScriptBuf,
    config: &DepositTxParams,
) -> Result<DepositTag<'a>, DepositParseError> {
    let mut instructions = script.instructions();

    // Check if OP_RETURN is present and if not just discard it.
    if next_op(&mut instructions) != Some(OP_RETURN) {
        return Err(DepositParseError::MissingTag);
    }

    // Extract the data from the next push.
    let Some(data) = next_bytes(&mut instructions) else {
        return Err(DepositParseError::NoData);
    };

    // If it's not a standard tx then something is *probably* up.
    if data.len() > 80 {
        return Err(DepositParseError::TagOversized);
    }

    parse_tag(data, config)
}

/// Parses the script buffer which has the following structure:
/// [magic_bytes(n bytes), stake_idx(4 bytes), ee_address(m bytes), takeback_hash(32 bytes),
/// sats_amt(8 bytes)]
fn parse_tag<'b>(
    buf: &'b [u8],
    config: &DepositTxParams,
) -> Result<DepositTag<'b>, DepositParseError> {
    // data has expected magic bytes
    let magic_bytes = &config.magic_bytes;
    let magic_len = magic_bytes.len();

    if buf.len() < magic_len + DEPOSIT_IDX_LEN + SATS_AMOUNT_LEN + TAKEBACK_HASH_LEN {
        return Err(DepositParseError::InvalidData);
    }

    let (magic_slice, idx_ee_takeback_amt) = buf.split_at(magic_len);
    if magic_slice != magic_bytes {
        return Err(DepositParseError::InvalidMagic);
    }

    // Extract the deposit idx. Can use expect because of the above length check
    let (didx_buf, ee_takeback_amt) = idx_ee_takeback_amt.split_at(DEPOSIT_IDX_LEN);
    let deposit_idx =
        u32::from_be_bytes(didx_buf.try_into().expect("Expect dep idx to be 4 bytes"));

    let (dest_buf, takeback_and_amt) =
        ee_takeback_amt.split_at(ee_takeback_amt.len() - SATS_AMOUNT_LEN - TAKEBACK_HASH_LEN);

    // Check dest_buf len
    if dest_buf.len() != config.address_length as usize {
        return Err(DepositParseError::InvalidDestLen(dest_buf.len() as u8));
    }

    // Extract takeback and amt
    let (takeback_hash, amt) = takeback_and_amt.split_at(TAKEBACK_HASH_LEN);

    // Extract sats, can use expect here because by the initial check on the buf len, we can ensure
    // this.
    let amt_bytes: [u8; 8] = amt
        .try_into()
        .expect("Expected to have 8 bytes as sats amount");

    let sats_amt = u64::from_be_bytes(amt_bytes);

    Ok(DepositTag {
        deposit_idx,
        dest_buf,
        amount: sats_amt,
        tapscript_root: takeback_hash
            .try_into()
            .expect("expected takeback hash length to match"),
    })
}

#[cfg(test)]
mod tests {

    use bitcoin::Amount;
    use strata_test_utils::bitcoin::{
        build_test_deposit_script, create_test_deposit_tx, test_taproot_addr,
    };

    use crate::deposit::{deposit_tx::extract_deposit_info, test_utils::get_deposit_tx_config};

    #[test]
    fn check_deposit_parser() {
        // values for testing
        let config = get_deposit_tx_config();
        let amt = Amount::from_sat(config.deposit_amount);
        let idx = 0xdeadbeef;
        let ee_addr = [1; 20];

        let deposit_request_script =
            build_test_deposit_script(config.magic_bytes, idx, ee_addr.to_vec());

        let test_transaction = create_test_deposit_tx(
            Amount::from_sat(config.deposit_amount),
            &test_taproot_addr().address().script_pubkey(),
            &deposit_request_script,
        );

        let out = extract_deposit_info(&test_transaction, &get_deposit_tx_config());

        assert!(out.is_some());
        let out = out.unwrap();

        assert_eq!(out.amt, amt.into());
        assert_eq!(out.deposit_idx, idx);
        assert_eq!(out.address, ee_addr);
    }
}
