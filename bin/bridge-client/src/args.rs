//! Parses command-line arguments for the bridge-client CLI.
use std::{fmt::Display, str::FromStr};

use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "express-bridge-client")]
#[command(about = "The bridge client for Express")]
pub(crate) struct Cli {
    #[arg(
        value_enum,
        help = "What mode to run the client in `Operator` (alias: op) or `Challenger` (alias: ch)"
    )]
    pub mode: OperationMode,
}

#[derive(Debug, Clone, ValueEnum, Parser)]
pub(super) enum OperationMode {
    /// Run client in Operator mode to handle deposits, withdrawals and challenging.
    #[clap(alias = "op")]
    Operator,

    /// Run client in Challenger mode to verify/challenge Operator claims.
    #[clap(alias = "ch")]
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

impl FromStr for OperationMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "operator" => Ok(Self::Operator),
            "challenger" => Ok(Self::Challenger),
            _ => Err("Invalid mode".to_string()),
        }
    }
}
