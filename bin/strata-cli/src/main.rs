pub mod cmd;
pub mod recovery;
pub mod rollup;
pub mod seed;
pub mod settings;
pub mod signet;
pub mod taproot;

use cmd::{
    backup::backup, balance::balance, bridge_in::bridge_in, bridge_out::bridge_out,
    change_pwd::change_pwd, drain::drain, faucet::faucet, receive::receive, refresh::refresh,
    reset::reset, send::send, Commands, TopLevel,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let TopLevel { cmd } = argh::from_env();
    match cmd {
        Commands::Refresh(_) => refresh().await,
        Commands::Drain(args) => drain(args).await,
        Commands::Balance(args) => balance(args).await,
        Commands::Backup(args) => backup(args).await,
        Commands::BridgeIn(args) => bridge_in(args).await,
        Commands::BridgeOut(args) => bridge_out(args).await,
        Commands::Faucet(args) => faucet(args).await,
        Commands::Send(args) => send(args).await,
        Commands::Receive(args) => receive(args).await,
        Commands::Reset(args) => reset(args).await,
        Commands::ChangePwd(args) => change_pwd(args).await,
    }
}
