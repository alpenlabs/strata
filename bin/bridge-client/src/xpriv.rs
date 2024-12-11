//! Parses the operator's master xpriv from a file.

use std::{
    env,
    fs::read_to_string,
    path::{Path, PathBuf},
    str::FromStr,
};

use bitcoin::bip32::Xpriv;
use strata_key_derivation::operator::OperatorKeys;
use strata_primitives::keys::ZeroizableXpriv;

/// The environment variable that contains the operator's master [`Xpriv`].
const OPXPRIV_ENVVAR: &str = "STRATA_OP_MASTER_XPRIV";

/// Parses the master [`Xpriv`] from a file.
pub(crate) fn parse_master_xpriv(path: &Path) -> anyhow::Result<OperatorKeys> {
    let xpriv: ZeroizableXpriv = Xpriv::from_str(read_to_string(path)?.as_ref())?.into();

    OperatorKeys::new(&xpriv).map_err(|_| anyhow::anyhow!("invalid master xpriv"))
}

/// Resolves the master [`Xpriv`] from ENV vars or CLI.
///
/// If the `--master-xpriv` CLI argument is provided, it will be used.
/// Otherwise, the `STRATA_OP_MASTER_XPRIV` ENV var will be used.
pub(crate) fn resolve_xpriv(
    cli_arg: Option<String>,
    cli_path: Option<String>,
) -> anyhow::Result<OperatorKeys> {
    match (cli_arg, cli_path) {
        (Some(xpriv), _) => OperatorKeys::new(&xpriv.parse::<Xpriv>()?)
            .map_err(|_| anyhow::anyhow!("invalid master xpriv from CLI")),

        (_, Some(path)) => parse_master_xpriv(&PathBuf::from(path)),

        (None, None) => match env::var(OPXPRIV_ENVVAR) {
            Ok(xpriv_env_str) => OperatorKeys::new(&xpriv_env_str.parse::<Xpriv>()?)
                .map_err(|_| anyhow::anyhow!("invalid master xpriv from envvar")),
            Err(_) => {
                anyhow::bail!(
                    "must either set {OPXPRIV_ENVVAR} envvar or pass with `--master-xpriv`"
                )
            }
        },
    }
}
