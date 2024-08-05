//! Parses command-line arguments for the bridge-client CLI.
use std::fmt::Display;
use std::str::FromStr;

use clap::builder;
use clap::builder::TypedValueParser;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "express-bridge-client")]
#[command(about = "The bridge client for Express")]
pub(crate) struct Args {
    #[arg(
        short = 'm',
        long = "mode",
        default_value_t = ModeOfOperation::Operator,
        value_parser = builder::PossibleValuesParser::new(["operator", "challenger"])
            .map(|s| ModeOfOperation::from_str(&s).unwrap())
    )]
    pub mode: ModeOfOperation,
}

#[derive(Debug, Clone)]
pub(super) enum ModeOfOperation {
    Operator,
    Challenger,
}

impl Display for ModeOfOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeOfOperation::Operator => write!(f, "operator"),
            ModeOfOperation::Challenger => write!(f, "challenger"),
        }
    }
}

impl FromStr for ModeOfOperation {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "operator" => Ok(Self::Operator),
            "challenger" => Ok(Self::Challenger),
            _ => Err("Invalid mode".to_string()),
        }
    }
}
