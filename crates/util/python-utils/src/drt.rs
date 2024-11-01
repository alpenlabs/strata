use std::str::FromStr;

use bdk_wallet::{
    bitcoin::{
        consensus::encode::serialize, hashes::Hash, secp256k1::SECP256K1, taproot::LeafVersion,
        Address, FeeRate, TapNodeHash, Transaction, XOnlyPublicKey,
    },
    descriptor::IntoWalletDescriptor,
    miniscript::{miniscript::Tap, Miniscript},
    template::DescriptorTemplateOut,
    KeychainKind, TxOrdering, Wallet,
};
use pyo3::prelude::*;
use reth_primitives::Address as RethAddress;

use crate::{
    constants::{BRIDGE_IN_AMOUNT, MAGIC_BYTES, NETWORK, RECOVER_DELAY, UNSPENDABLE},
    error::Error,
    parse::{parse_el_address, parse_xonly_pk},
    taproot::{new_client, sync_wallet, taproot_wallet, ExtractP2trPubkey},
};

/// Generates a deposit request transaction (DRT).
///
/// # Arguments
///
/// - `el_address`: Execution layer address of the account that will receive the funds.
/// - `musig_bridge_pk`: MuSig bridge X-only public key.
/// - `bitcoind_url`: URL of the `bitcoind` instance.
/// - `bitcoind_user`: Username for the `bitcoind` instance.
/// - `bitcoind_password`: Password for the `bitcoind` instance.
///
/// # Returns
///
/// A signed (with the `private_key`) and serialized Deposit Request transaction.
#[pyfunction]
pub(crate) fn deposit_request_transaction(
    el_address: String,
    musig_bridge_pk: String,
    bitcoind_url: String,
    bitcoind_user: String,
    bitcoind_password: String,
) -> PyResult<Vec<u8>> {
    let signed_tx = deposit_request_transaction_inner(
        el_address.as_str(),
        musig_bridge_pk.as_str(),
        bitcoind_url.as_str(),
        bitcoind_user.as_str(),
        bitcoind_password.as_str(),
    )?;
    let signed_tx = serialize(&signed_tx);
    Ok(signed_tx)
}

/// Generates a deposit request transaction (DRT).
///
/// # Arguments
///
/// - `el_address`: Execution layer address of the account that will receive the funds.
/// - `musig_bridge_pk`: MuSig bridge X-only public key.
/// - `bitcoind_url`: URL of the `bitcoind` instance.
/// - `bitcoind_user`: Username for the `bitcoind` instance.
/// - `bitcoind_password`: Password for the `bitcoind` instance.
///
/// # Returns
///
/// A signed (with the `private_key`) and serialized Deposit Request transaction.
fn deposit_request_transaction_inner(
    el_address: &str,
    musig_bridge_pk: &str,
    bitcoind_url: &str,
    bitcoind_user: &str,
    bitcoind_password: &str,
) -> Result<Transaction, Error> {
    // Parse stuff
    let el_address = parse_el_address(el_address)?;
    let musig_bridge_pk = parse_xonly_pk(musig_bridge_pk)?;

    // Instantiate the BitcoinD client
    let client = new_client(
        bitcoind_url,
        None,
        Some(bitcoind_user),
        Some(bitcoind_password),
    )?;

    // Get the address and the bridge descriptor
    let mut wallet = taproot_wallet()?;
    let recovery_address = wallet.reveal_next_address(KeychainKind::External).address;
    let (bridge_in_desc, recovery_script_hash) =
        bridge_in_descriptor(musig_bridge_pk, recovery_address)
            .expect("valid bridge in descriptor");

    let desc = bridge_in_desc
        .clone()
        .into_wallet_descriptor(SECP256K1, NETWORK)
        .expect("valid descriptor");

    let mut temp_wallet = Wallet::create_single(desc.clone())
        .network(NETWORK)
        .create_wallet_no_persist()
        .expect("valid wallet");

    let bridge_in_address = temp_wallet
        .reveal_next_address(KeychainKind::External)
        .address;

    // Magic bytes + TapNodeHash + Recovery Address
    const MBL: usize = MAGIC_BYTES.len();
    const TNHL: usize = TapNodeHash::LEN;
    let mut op_return_data = [0u8; MBL + TNHL + RethAddress::len_bytes()];
    op_return_data[..MBL].copy_from_slice(MAGIC_BYTES);
    op_return_data[MBL..MBL + TNHL]
        .copy_from_slice(recovery_script_hash.as_raw_hash().as_byte_array());
    op_return_data[MBL + TNHL..].copy_from_slice(el_address.as_slice());

    // For regtest 2 sat/vbyte is enough
    let fee_rate = FeeRate::from_sat_per_vb(2).expect("valid fee rate");

    // Before signing the transaction, we need to sync the wallet with bitcoind
    sync_wallet(&mut wallet, &client)?;

    let mut psbt = wallet
        .build_tx()
        // NOTE: the deposit won't be found by the sequencer if the order isn't correct.
        .ordering(TxOrdering::Untouched)
        .add_recipient(bridge_in_address.script_pubkey(), BRIDGE_IN_AMOUNT)
        .add_data(&op_return_data)
        .fee_rate(fee_rate)
        .clone()
        .finish()
        .expect("valid psbt");
    wallet.sign(&mut psbt, Default::default()).unwrap();

    let tx = psbt.extract_tx().expect("valid tx");
    Ok(tx)
}

/// The descriptor for the bridge-in transaction.
///
/// # Note
///
/// The descriptor is a Tapscript that enforces the following conditions:
///
/// - The funds can be spent by the bridge operator.
/// - The funds can be spent by the recovery address after a delay.
///
/// # Returns
///
/// The descriptor and the script hash for the recovery path.
fn bridge_in_descriptor(
    bridge_pubkey: XOnlyPublicKey,
    recovery_address: Address,
) -> Result<(DescriptorTemplateOut, TapNodeHash), Error> {
    let recovery_xonly_pubkey = recovery_address.extract_p2tr_pubkey()?;

    let desc = bdk_wallet::descriptor!(
        tr(UNSPENDABLE, {
            pk(bridge_pubkey),
            and_v(v:pk(recovery_xonly_pubkey),older(RECOVER_DELAY))
        })
    )
    .expect("valid descriptor");

    // we have to do this to obtain the script hash
    // i have tried to extract it directly from the desc above
    // it is a massive pita
    let recovery_script = Miniscript::<XOnlyPublicKey, Tap>::from_str(&format!(
        "and_v(v:pk({}),older(1008))",
        recovery_xonly_pubkey
    ))
    .expect("valid recovery script")
    .encode();

    let recovery_script_hash = TapNodeHash::from_script(&recovery_script, LeafVersion::TapScript);

    Ok((desc, recovery_script_hash))
}

#[cfg(test)]
mod tests {
    use bdk_wallet::KeychainKind;
    use bitcoind::{bitcoincore_rpc::RpcApi, BitcoinD};
    use strata_btcio::rpc::{traits::Broadcaster, BitcoinClient};
    use strata_common::logging;

    use super::*;
    use crate::taproot::taproot_wallet;

    const EL_ADDRESS: &str = "deedf001900dca3ebeefdeadf001900dca3ebeef";
    const MUSIG_BRIDGE_PK: &str =
        "14ced579c6a92533fa68ccc16da93b41073993cfc6cc982320645d8e9a63ee65";

    /// Get the authentication credentials for a given `bitcoind` instance.
    fn get_auth(bitcoind: &BitcoinD) -> (String, String) {
        let params = &bitcoind.params;
        let cookie_values = params.get_cookie_values().unwrap().unwrap();
        (cookie_values.user, cookie_values.password)
    }

    /// Mine a number of blocks of a given size `count`, which may be specified to a given coinbase
    /// `address`.
    fn mine_blocks(
        bitcoind: &BitcoinD,
        count: usize,
        address: Option<Address>,
    ) -> anyhow::Result<()> {
        let coinbase_address = match address {
            Some(address) => address,
            None => bitcoind
                .client
                .get_new_address(None, None)?
                .assume_checked(),
        };
        let _ = bitcoind
            .client
            .generate_to_address(count as _, &coinbase_address)?;
        Ok(())
    }

    #[tokio::test]
    async fn drt_mempool_accept() {
        logging::init(logging::LoggerConfig::with_base_name("drt-tests"));

        let bitcoind = BitcoinD::from_downloaded().unwrap();
        let url = bitcoind.rpc_url();
        let (user, password) = get_auth(&bitcoind);
        let client = BitcoinClient::new(url.clone(), user.clone(), password.clone()).unwrap();

        let mut wallet = taproot_wallet().unwrap();
        let address = wallet.reveal_next_address(KeychainKind::External).address;

        // Mine and get the last UTXO which should have 50 BTC.
        mine_blocks(&bitcoind, 101, Some(address)).unwrap();

        let signed_tx =
            deposit_request_transaction_inner(EL_ADDRESS, MUSIG_BRIDGE_PK, &url, &user, &password)
                .unwrap();

        let txid = client.send_raw_transaction(&signed_tx).await.unwrap();

        assert_eq!(txid, signed_tx.compute_txid());
    }
}
