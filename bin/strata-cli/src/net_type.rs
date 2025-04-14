use std::{fmt, str::FromStr};

use crate::errors::{CliError, UserInputError};

/// Represents a type of network, either Alpen's signet or Strata
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum NetworkType {
    Signet,
    Strata,
}

impl FromStr for NetworkType {
    type Err = CliError;

    fn from_str(s: &str) -> Result<Self, CliError> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "strata" => Ok(Self::Strata),
            _ => Err(CliError::UserInput(UserInputError::UnsupportedNetwork)),
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

/// Parses `val` as a [`NetworkType`]. Prints error message and exits if invalid.
pub fn parse_net_type(val: &str) -> Result<NetworkType, CliError> {
    val.parse()
}
