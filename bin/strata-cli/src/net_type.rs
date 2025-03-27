use std::str::FromStr;

/// Represents a type of network, either Alpen's signet or Alpen
#[derive(PartialEq, Eq)]
pub enum NetworkType {
    Signet,
    Alpen,
}

/// Attempted to parse a string into [`NetworkType`] but the input was invalid.
pub struct InvalidNetworkType;

impl FromStr for NetworkType {
    type Err = InvalidNetworkType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "signet" => Ok(Self::Signet),
            "alpen" => Ok(Self::Alpen),
            _ => Err(InvalidNetworkType),
        }
    }
}

/// Parses `val` as a [`NetworkType`]. Prints error message and exits if invalid.
pub fn net_type_or_exit(val: &str) -> NetworkType {
    match NetworkType::from_str(val) {
        Ok(t) => t,
        Err(InvalidNetworkType) => {
            println!("Invalid network type. Must be signet or alpen");
            std::process::exit(1)
        }
    }
}
