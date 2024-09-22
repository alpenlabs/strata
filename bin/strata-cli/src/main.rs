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
use seed::Seed;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let TopLevel { cmd } = argh::from_env();
    let seed = Seed::load_or_create().unwrap();

    match cmd {
        Commands::Refresh(_) => refresh(seed).await,
        Commands::Drain(args) => drain(args).await,
        Commands::Balance(args) => balance(args, seed).await,
        Commands::Backup(args) => backup(args, seed).await,
        Commands::BridgeIn(args) => bridge_in(args, seed).await,
        Commands::BridgeOut(args) => bridge_out(args, seed).await,
        Commands::Faucet(args) => faucet(args, seed).await,
        Commands::Send(args) => send(args, seed).await,
        Commands::Receive(args) => receive(args, seed).await,
        Commands::Reset(args) => reset(args).await,
        Commands::ChangePwd(args) => change_pwd(args, seed).await,
    }
}
