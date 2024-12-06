//! Parses the operator's master xpriv from a file.

use std::path::Path;

use bitcoin::bip32::Xpriv;
use strata_key_derivation::operator::OperatorKeys;

pub(crate) fn parse_master_xpriv(path: &Path) -> anyhow::Result<OperatorKeys> {
    let xpriv = std::fs::read_to_string(path)?;
    let xpriv = xpriv.parse::<Xpriv>()?;
    OperatorKeys::new(&xpriv).map_err(|_| anyhow::anyhow!("invalid master xpriv"))
}
