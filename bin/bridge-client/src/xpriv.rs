//! Parses the operator's master xpriv from a file.

use std::fs::read_to_string;

use bitcoin::bip32::Xpriv;
use strata_key_derivation::operator::OperatorKeys;
use tracing::*;
use zeroize::Zeroize;

/// The environment variable that contains the operator's master [`Xpriv`].
pub const OPXPRIV_ENVVAR: &str = "STRATA_OP_MASTER_XPRIV";

/// Resolves the master [`Xpriv`] from the various sources.
///
/// Rules:
///
/// 1. If none are set, error out.
/// 2. If multiple are set, error out.
/// 3. If we have the verbatim key provided, parse it.
/// 4. If we have a path provided, load it and parse that instead.
///
/// # Errors
///
/// Returns an error if the master xpriv is invalid or not found, or if
/// conflicting options are set.
pub(crate) fn resolve_xpriv(
    cli_arg: Option<String>,
    cli_path: Option<String>,
    env_val: Option<String>,
) -> anyhow::Result<OperatorKeys> {
    if cli_arg.is_some() {
        error!("FOUND CLI ARG KEY, THIS IS INSECURE!");
    }

    let mut xpriv_str: String = match (cli_arg, cli_path, env_val) {
        // If there's none set then we error out.
        (None, None, None) => {
            anyhow::bail!(
                "must provide root xpriv with either `--master-xpriv-path` or {OPXPRIV_ENVVAR}"
            )
        }

        // If multiple are set then we error out.
        (_, Some(_), Some(_)) | (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            anyhow::bail!("multiple root xpriv options specified, don't know what to do, aborting");
        }

        // In these cases we have the string explicitly.
        (Some(xpriv_str), _, _) | (_, _, Some(xpriv_str)) => xpriv_str.to_owned(),

        // In this case we fetch it from file.
        (_, Some(path), _) => read_to_string(path)?,
    };

    // Some fancy dance to securely erase things.
    let Ok(raw) = xpriv_str.parse::<Xpriv>() else {
        xpriv_str.zeroize();
        anyhow::bail!("invalid master xpriv");
    };

    let Ok(keys) = OperatorKeys::new(&raw) else {
        xpriv_str.zeroize();
        // TODO how to secure erase raw?
        anyhow::bail!("unable to generate leaf keys");
    };

    Ok(keys)
}
