//! Bridge Operator client.
//!
//! Responsible for facilitating bridge-in and bridge-out operations by creating, storing and
//! publishing appropriate transactions. Can also perform challenger duties.

mod args;
pub(crate) mod constants;
mod modes;
pub(crate) mod rpc_server;

use alpen_express_common::logging;
use args::{Args, ModeOfOperation};
use clap::Parser;
use modes::{challenger, operator};
use tracing::info;

#[tokio::main]
async fn main() {
    logging::init();

    let cli_args = Args::parse();

    info!("running bridge client in {} mode", cli_args.mode);

    match cli_args.mode {
        ModeOfOperation::Operator => {
            operator::bootstrap()
                .await
                .expect("bootstrap operator node");
        }
        ModeOfOperation::Challenger => {
            challenger::bootstrap().await;
        }
    }
}
