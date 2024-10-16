use std::path::PathBuf;

use bdk_bitcoind_rpc::{
    bitcoincore_rpc::{Auth, Client},
    Emitter,
};
use bdk_wallet::{
    bitcoin::{key::Parity, Address, AddressType, XOnlyPublicKey},
    KeychainKind, Wallet,
};
use musig2::KeyAggContext;
use pyo3::prelude::*;

use crate::{
    constants::{CHANGE_DESCRIPTOR, DESCRIPTOR, NETWORK},
    error::Error,
    parse::parse_xonly_pk,
};

/// Extracts the public key from a Taproot address.
pub(crate) trait ExtractP2trPubkey {
    /// Extracts the public key from a Taproot address.
    fn extract_p2tr_pubkey(&self) -> Result<XOnlyPublicKey, Error>;
}

impl ExtractP2trPubkey for Address {
    fn extract_p2tr_pubkey(&self) -> Result<XOnlyPublicKey, Error> {
        match self.address_type() {
            Some(AddressType::P2tr) => {}
            _ => return Err(Error::NotTaprootAddress),
        }

        let script_pubkey = self.script_pubkey();

        Ok(XOnlyPublicKey::from_slice(&script_pubkey.as_bytes()[2..]).expect("valid pub key"))
    }
}

/// A simple Taproot-enable wallet.
///
/// # Note
///
/// This uses the hardcoded `[DESCRIPTOR]` and `[CHANGE_DESCRIPTOR]`.
pub(crate) fn taproot_wallet() -> Result<Wallet, Error> {
    Ok(Wallet::create(*DESCRIPTOR, *CHANGE_DESCRIPTOR)
        .network(NETWORK)
        .create_wallet_no_persist()
        .map_err(|_| Error::Wallet))?
}

/// Syncs a wallet with the network using `bitcoind` as the backend.
///
/// # Note
///
/// This function should be only used with Regtest.
pub(crate) fn sync_wallet(wallet: &mut Wallet, rpc_client: Client) -> Result<(), Error> {
    let wallet_tip = wallet.latest_checkpoint();
    let mut emitter = Emitter::new(&rpc_client, wallet_tip, 0);
    while let Some(block) = emitter.next_block().expect("valid block") {
        let height = block.block_height();
        let connected_to = block.connected_to();
        wallet
            .apply_block_connected_to(&block.block, height, connected_to)
            .map_err(|_| Error::BitcoinD)?
    }
    Ok(())
}

/// Creates a new `bitcoind` RPC client.
pub(crate) fn new_client(
    url: &str,
    rpc_cookie: Option<&PathBuf>,
    rpc_user: Option<&str>,
    rpc_pass: Option<&str>,
) -> Result<Client, Error> {
    Ok(Client::new(
        url,
        match (rpc_cookie, rpc_user, rpc_pass) {
            (None, None, None) => Auth::None,
            (Some(path), _, _) => Auth::CookieFile(path.clone()),
            (_, Some(user), Some(pass)) => Auth::UserPass(user.into(), pass.into()),
            (_, Some(_), None) => panic!("rpc auth: missing rpc_pass"),
            (_, None, Some(_)) => panic!("rpc auth: missing rpc_user"),
        },
    )
    .map_err(|_| Error::RpcClient))?
}

/// MuSig2 aggregates public keys into a single public key.
///
/// # Note
///
/// These should all be X-only public keys, assuming that all are [`Parity::Even`].
pub(crate) fn musig_aggregate_pks_inner(pks: Vec<XOnlyPublicKey>) -> Result<XOnlyPublicKey, Error> {
    let pks: Vec<(XOnlyPublicKey, Parity)> = pks.into_iter().map(|pk| (pk, Parity::Even)).collect();
    let key_agg_ctx = KeyAggContext::new(pks).map_err(|_| Error::XOnlyPublicKey)?;
    Ok(key_agg_ctx.aggregated_pubkey())
}

/// Gets a (receiving/external) address from the wallet at the given `index`.
#[pyfunction]
pub(crate) fn get_address(index: u32) -> Result<String, Error> {
    let wallet = taproot_wallet()?;
    let address = wallet
        .peek_address(KeychainKind::External, index)
        .address
        .to_string();
    Ok(address)
}

/// Gets a (change/internal) address from the wallet at a given `index`.
#[pyfunction]
pub(crate) fn get_change_address(index: u32) -> Result<String, Error> {
    let wallet = taproot_wallet()?;
    let address = wallet
        .peek_address(KeychainKind::Internal, index)
        .address
        .to_string();
    Ok(address)
}

/// MuSig2 aggregates public keys into a single public key.
///
/// # Note
///
/// These should all be X-only public keys.
#[pyfunction]
pub(crate) fn musig_aggregate_pks(pks: Vec<String>) -> Result<String, Error> {
    let pks = pks
        .into_iter()
        .map(|pk| parse_xonly_pk(&pk).map_err(|_| Error::XOnlyPublicKey))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(musig_aggregate_pks_inner(pks)?.to_string())
}

/// Extract the [`XOnlyPublicKey`] from a Taproot `address`.
///
/// # Note
///
/// This assumes that the caller has verified the `address`.
#[pyfunction]
pub(crate) fn extract_p2tr_pubkey(address: String) -> Result<String, Error> {
    let address = &address
        .parse::<Address<_>>()
        .map_err(|_| Error::NotTaprootAddress)?
        .assume_checked();
    let pk = address.extract_p2tr_pubkey()?;
    Ok(pk.to_string())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bdk_wallet::KeychainKind;
    use shrex::hex;

    use super::*;

    #[test]
    fn extract_p2tr_pubkey() {
        let address =
            Address::from_str("bcrt1phcnl4zcl2fu047pv4wx6y058v8u0n02at6lthvm7pcf2wrvjm5tqatn90k")
                .unwrap()
                .require_network(NETWORK)
                .unwrap();

        let pk = address.extract_p2tr_pubkey().unwrap();
        let expected = XOnlyPublicKey::from_slice(&hex!(
            "be27fa8b1f5278faf82cab8da23e8761f8f9bd5d5ebebbb37e0e12a70d92dd16"
        ))
        .unwrap();
        assert_eq!(pk, expected);
    }

    #[test]
    fn taproot_wallet() {
        let mut wallet = super::taproot_wallet().unwrap();

        let address = wallet
            .reveal_next_address(KeychainKind::External)
            .to_string();
        let expected = "bcrt1phcnl4zcl2fu047pv4wx6y058v8u0n02at6lthvm7pcf2wrvjm5tqatn90k";
        assert_eq!(address, expected);

        let change_address = wallet
            .reveal_next_address(KeychainKind::Internal)
            .to_string();
        let expected = "bcrt1pz449kexzydh2kaypatup5ultru3ej284t6eguhnkn6wkhswt0l7q3a7j76";
        assert_eq!(change_address, expected);
    }

    #[test]
    fn musig_aggregate_pks() {
        let pks: [XOnlyPublicKey; 2] = [
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
                .parse()
                .unwrap(),
            "3590a94e768f8e1815c2f24b4d80a8e3149316c3518ce7b7ad338368d038ca66"
                .parse()
                .unwrap(),
        ];
        let aggregated_pk = musig_aggregate_pks_inner(pks.to_vec()).unwrap().to_string();
        let expected = "85eb6101982e142dba553cae437d08a82880fe9a22889c997f8e415a61b7a2d5";
        assert_eq!(aggregated_pk, expected);
    }
}
