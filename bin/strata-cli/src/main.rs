pub mod cmd;
pub mod constants;
pub mod net_type;
pub mod recovery;
pub mod seed;
pub mod settings;
pub mod signet;
pub mod strata;
pub mod taproot;

use cmd::{
    backup::backup, balance::balance, bridge_in::bridge_in, bridge_out::bridge_out,
    change_pwd::change_pwd, config::config, drain::drain, faucet::faucet, receive::receive,
    recover::recover, reset::reset, scan::scan, send::send, Commands, TopLevel,
};
#[cfg(target_os = "linux")]
use seed::FilePersister;
#[cfg(not(target_os = "linux"))]
use seed::KeychainPersister;
use settings::Settings;
use signet::{set_data_dir, EsploraClient};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let TopLevel { cmd } = argh::from_env();
    let settings = Settings::load().unwrap();

    if let Commands::Config(args) = cmd {
        config(args, settings).await;
        return;
    }

    #[cfg(not(target_os = "linux"))]
    let persister = KeychainPersister;
    #[cfg(target_os = "linux")]
    let persister = FilePersister::new(settings.linux_seed_file.clone());

    assert!(set_data_dir(settings.data_dir.clone()));

    let seed = seed::load_or_create(&persister).unwrap();
    let esplora = EsploraClient::new(&settings.esplora).expect("valid esplora url");

    match cmd {
        Commands::Recover(_) => recover(seed, settings, esplora).await,
        Commands::Drain(args) => drain(args, seed, settings, esplora).await,
        Commands::Balance(args) => balance(args, seed, settings, esplora).await,
        Commands::Backup(args) => backup(args, seed).await,
        Commands::BridgeIn(args) => bridge_in(args, seed, settings, esplora).await,
        Commands::BridgeOut(args) => bridge_out(args, seed, settings).await,
        Commands::Faucet(args) => faucet(args, seed, settings).await,
        Commands::Send(args) => send(args, seed, settings, esplora).await,
        Commands::Receive(args) => receive(args, seed, settings, esplora).await,
        Commands::Reset(args) => reset(args, persister, settings).await,
        Commands::ChangePwd(args) => change_pwd(args, seed, persister).await,
        Commands::Scan(args) => scan(args, seed, settings, esplora).await,
        _ => {}
    }
}
