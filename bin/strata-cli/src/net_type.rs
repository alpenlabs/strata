use std::{fmt, str::FromStr};

/// Represents a type of network, either Alpen's signet or Strata
#[non_exhaustive]
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum NetworkType {
    Signet,
    Strata,
}

#[derive(Clone, Copy, Debug)]
pub struct InvalidNetwork;

impl FromStr for NetworkType {
    type Err = InvalidNetwork;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "strata" => Ok(Self::Strata),
            _ => Err(InvalidNetwork),
        }
    }
}

impl fmt::Display for NetworkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NetworkType::Signet => "signet",
            NetworkType::Strata => "strata",
        })
    }
}
