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
#[cfg(target_os = "linux")]
use seed::FilePersister;
#[cfg(not(target_os = "linux"))]
use seed::KeychainPersister;
#[cfg(target_os = "linux")]
use settings::SETTINGS;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let TopLevel { cmd } = argh::from_env();
    #[cfg(not(target_os = "linux"))]
    let persister = KeychainPersister;
    #[cfg(target_os = "linux")]
    let persister = FilePersister::new(SETTINGS.linux_seed_file.clone());
    let seed = seed::load_or_create(&persister).unwrap();

    match cmd {
        Commands::Refresh(_) => refresh(seed).await,
        Commands::Drain(args) => drain(args, seed).await,
        Commands::Balance(args) => balance(args, seed).await,
        Commands::Backup(args) => backup(args, seed).await,
        Commands::BridgeIn(args) => bridge_in(args, seed).await,
        Commands::BridgeOut(args) => bridge_out(args, seed).await,
        Commands::Faucet(args) => faucet(args, seed).await,
        Commands::Send(args) => send(args, seed).await,
        Commands::Receive(args) => receive(args, seed).await,
        Commands::Reset(args) => reset(args, persister).await,
        Commands::ChangePwd(args) => change_pwd(args, seed, persister).await,
    }
}
