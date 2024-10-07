use std::str::FromStr;

use console::Term;

pub enum NetworkType {
    Signet,
    Strata,
}

pub struct InvalidNetworkType;

impl FromStr for NetworkType {
    type Err = InvalidNetworkType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "strata" => Ok(Self::Strata),
            _ => Err(InvalidNetworkType),
        }
    }
}

/// Parses `val` as a [`NetworkType`]. Prints error message and exits if invalid.
pub fn net_type_or_exit(val: &str, term: &Term) -> NetworkType {
    match NetworkType::from_str(val) {
        Ok(t) => t,
        Err(InvalidNetworkType) => {
            let _ = term.write_line("Invalid network type. Must be signet or strata");
            std::process::exit(1)
        }
    }
}
