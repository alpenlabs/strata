//! Descriptor parsing utilities.

use std::env;

use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv},
    secp256k1::SECP256K1,
};
use miniscript::descriptor::{checksum::desc_checksum, InnerXKey};
use strata_btcio::rpc::{traits::Signer, types::ImportDescriptor};

// TODO: move some of these into a keyderiv crate
const DERIV_BASE_IDX: u32 = 56;
const DERIV_OP_IDX: u32 = 20;
const DERIV_OP_SIGNING_IDX: u32 = 100;
const DERIV_OP_WALLET_IDX: u32 = 101;
const OPXPRIV_ENVVAR: &str = "STRATA_OP_XPRIV";
#[allow(unused)] // TODO: uncomment when we need to store the xpriv directly in the wallet.
const WALLET_NAME: &str = "strata";

/// Resolves a key from ENV vars or CLI.
pub(crate) fn resolve_xpriv(cli_arg: Option<String>) -> anyhow::Result<Xpriv> {
    match cli_arg {
        Some(xpriv_str) => Ok(xpriv_str.parse::<Xpriv>().expect("could not parse xpriv")),

        None => match env::var(OPXPRIV_ENVVAR) {
            Ok(xpriv_env_str) => Ok(xpriv_env_str
                .parse::<Xpriv>()
                .expect("could not parse xpriv")),
            Err(_) => anyhow::bail!(
                "please specify either the ENV var {OPXPRIV_ENVVAR} or pass it as a CLI argument"
            ),
        },
    }
}

/// Derives the signing and wallet xprivs for a Strata bridge client.
pub(crate) fn derive_op_purpose_xprivs(master: &Xpriv) -> anyhow::Result<(Xpriv, Xpriv)> {
    let signing_path = DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_normal_idx(DERIV_OP_SIGNING_IDX).unwrap(),
    ]);

    let wallet_path = DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_normal_idx(DERIV_OP_WALLET_IDX).unwrap(),
    ]);

    let signing_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &signing_path)?;
    let wallet_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &wallet_path)?;

    Ok((signing_xpriv, wallet_xpriv))
}

/// Parses an [`Xpriv`] into a **Taproot** descriptor ready to be imported by Bitcoin core.
///
/// # Note
///
/// The current descriptor handling is not easy to do as a type,
/// hence this does all internals checks and then returns the descriptor
/// as a [`String`].
#[allow(unused)] // TODO: uncomment when we need to store the xpriv directly in the wallet.
pub(crate) fn generate_descriptor_from_xpriv(xpriv: Xpriv) -> anyhow::Result<String> {
    let hardened_path = DerivationPath::master().extend([
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).expect("bad child index"),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).expect("bad child index"),
    ]);

    let normal_path = DerivationPath::master()
        .extend([ChildNumber::from_normal_idx(DERIV_OP_WALLET_IDX).expect("bad child index")]);

    let fingerprint = xpriv.xkey_fingerprint(SECP256K1);
    let descriptor_str = format!("tr([{fingerprint}/{hardened_path}]{xpriv}/{normal_path}/*)");
    let checksum = desc_checksum(&descriptor_str).expect("could not calculate descriptor checksum");
    // tr([fingerprint/hardened_path]/normal_path/*)#checksum
    let descriptor_str = format!("{descriptor_str}#{checksum}");

    Ok(descriptor_str)
}

/// Checks if the wallet has the descriptor or loads it into the wallet.
#[allow(unused)] // TODO: uncomment when we need to store the xpriv directly in the wallet.
pub(crate) async fn check_or_load_descriptor_into_wallet(
    l1_client: &impl Signer,
    xpriv: Xpriv,
) -> anyhow::Result<()> {
    let xpriv_from_signer = l1_client
        .get_xpriv()
        .await
        .expect("could not get the listdescriptors call from the bitcoin RPC")
        .expect("could not get a valid xpriv from the bitcoin wallet");
    if xpriv == xpriv_from_signer {
        // nothing to do
        Ok(())
    } else {
        // load the descriptor
        let descriptor = generate_descriptor_from_xpriv(xpriv)?;
        let descriptors = vec![ImportDescriptor {
            desc: descriptor,
            active: Some(true),
            timestamp: "now".to_owned(),
        }];
        let result = l1_client
            .import_descriptors(descriptors, WALLET_NAME.to_string())
            .await
            .expect("could not get the importdescriptors call from the bitcoin RPC");
        assert!(
            result.iter().all(|r| r.success),
            "could not import xpriv as a descriptor into the wallet"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bitcoind::BitcoinD;
    use strata_btcio::rpc::{
        traits::Signer,
        types::{ImportDescriptor, ImportDescriptorResult},
        BitcoinClient,
    };
    use strata_common::logging;

    use super::*;

    // taken from https://github.com/rust-bitcoin/rust-bitcoin/blob/bb38aeb786f408247d5bbc88b9fa13616c74c009/bitcoin/examples/taproot-psbt.rs#L18C38-L18C149
    const XPRIV_STR: &str = "tprv8ZgxMBicQKsPd4arFr7sKjSnKFDVMR2JHw9Y8L9nXN4kiok4u28LpHijEudH3mMYoL4pM5UL9Bgdz2M4Cy8EzfErmU9m86ZTw6hCzvFeTg7";

    /// Get the authentication credentials for a given [`bitcoind`] instance.
    fn get_auth(bitcoind: &BitcoinD) -> (String, String) {
        let params = &bitcoind.params;
        let cookie_values = params.get_cookie_values().unwrap().unwrap();
        (cookie_values.user, cookie_values.password)
    }

    #[test]
    fn parse_xpriv_to_descriptor_string() {
        let expected = "tr([e61b318f/56'/20']tprv8ZgxMBicQKsPd4arFr7sKjSnKFDVMR2JHw9Y8L9nXN4kiok4u28LpHijEudH3mMYoL4pM5UL9Bgdz2M4Cy8EzfErmU9m86ZTw6hCzvFeTg7/101/*)#zz430whl";
        let xpriv = XPRIV_STR.parse::<Xpriv>().unwrap();
        let got = generate_descriptor_from_xpriv(xpriv).unwrap();
        assert_eq!(got, expected);
    }

    #[tokio::test]
    async fn bitcoind_import_descriptors() {
        logging::init();
        let bitcoind = BitcoinD::from_downloaded().unwrap();
        let url = bitcoind.rpc_url();
        let (user, password) = get_auth(&bitcoind);
        let client = BitcoinClient::new(url, user, password).unwrap();

        let xpriv = XPRIV_STR.parse::<Xpriv>().unwrap();
        let descriptor_string = generate_descriptor_from_xpriv(xpriv).unwrap();
        let timestamp = "now".to_owned();
        let list_descriptors = vec![ImportDescriptor {
            desc: descriptor_string,
            active: Some(true),
            timestamp,
        }];
        let got = client
            .import_descriptors(list_descriptors, WALLET_NAME.to_string())
            .await
            .unwrap();
        let expected = vec![ImportDescriptorResult { success: true }];
        assert_eq!(expected, got);
    }
}
