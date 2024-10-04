//! Bridge Operator client.
//!
//! Responsible for facilitating bridge-in and bridge-out operations by creating, storing and
//! publishing appropriate transactions. Can also perform challenger duties.

mod args;
pub(crate) mod constants;
pub(crate) mod db;
mod descriptor;
mod errors;
mod modes;
pub(crate) mod rpc_server;

use alpen_express_common::logging;
use args::{Cli, OperationMode};
use modes::{challenger, operator};
use tracing::info;

#[tokio::main]
async fn main() {
    logging::init();

    let cli_args: Cli = argh::from_env();

    let mode: OperationMode = match cli_args.mode.clone().try_into() {
        Ok(mode) => mode,
        Err(err) => {
            panic!("{}", err);
        }
    };

    info!("running bridge client in {} mode", mode);

    match mode {
        OperationMode::Operator => {
            operator::bootstrap(cli_args)
                .await
                .expect("bootstrap operator node");
        }
        OperationMode::Challenger => {
            challenger::bootstrap().await;
        }
    }
}
