use std::{fmt, str::FromStr};

use crate::errors::{user_error, DisplayedError};

/// Represents a type of network, either Alpen's signet or Strata
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum NetworkType {
    Signet,
    Strata,
}

impl FromStr for NetworkType {
    type Err = DisplayedError;

    fn from_str(s: &str) -> Result<Self, DisplayedError> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "strata" => Ok(Self::Strata),
            _ => Err(user_error(format!(
                "Unsupported network: '{}'. Must be `signet` or `strata`.",
                s
            ))),
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
