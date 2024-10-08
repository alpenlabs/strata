use std::str::FromStr;

use console::Term;

/// Represents a type of network, either Alpen's signet or Strata
#[derive(PartialEq)]
pub enum NetworkType {
    Signet,
    Strata,
}

/// Attempted to parse a string into [`NetworkType`] but the input was invalid.
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
