use std::{fmt, str::FromStr};

use crate::errors::UnsupportedNetwork;

/// Represents a type of network, either Alpen's signet or Strata
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum NetworkType {
    Signet,
    Strata,
}

impl FromStr for NetworkType {
    type Err = UnsupportedNetwork;

    fn from_str(s: &str) -> Result<Self, UnsupportedNetwork> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "strata" => Ok(Self::Strata),
            _ => Err(UnsupportedNetwork(s.to_string())),
        }
    }
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let net_str = match self {
            NetworkType::Signet => "signet",
            NetworkType::Strata => "strata",
        };
        write!(f, "{}", net_str)
    }
}
