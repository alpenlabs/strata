//! Parses command-line arguments for the bridge-client CLI.
use std::fmt::Display;

use argh::FromArgs;

use crate::errors::InitError;

#[derive(Debug, FromArgs)]
#[argh(name = "express-bridge-client")]
#[argh(description = "The bridge client for Express")]
pub(crate) struct Cli {
    #[argh(
        positional,
        description = "what mode to run the client in, either Operator (alias: op) or Challenger (alias: ch)"
    )]
    pub mode: String,
}

#[derive(Debug, Clone)]
pub(super) enum OperationMode {
    /// Run client in Operator mode to handle deposits, withdrawals and challenging.
    Operator,

    /// Run client in Challenger mode to verify/challenge Operator claims.
    Challenger,
}

impl Display for OperationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationMode::Operator => write!(f, "operator"),
            OperationMode::Challenger => write!(f, "challenger"),
        }
    }
}

impl TryInto<OperationMode> for String {
    type Error = InitError;

    fn try_into(self) -> Result<OperationMode, Self::Error> {
        match self.as_ref() {
            "operator" | "op" => Ok(OperationMode::Operator),
            "challenger" | "ch" => Ok(OperationMode::Challenger),
            other => Err(InitError::InvalidMode(other.to_string())),
        }
    }
}
