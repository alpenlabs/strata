use argh::FromArgs;
use backup::BackupArgs;
use balance::BalanceArgs;
use bridge_in::BridgeInArgs;
use bridge_out::BridgeOutArgs;
use change_pwd::ChangePwdArgs;
use drain::DrainArgs;
use faucet::FaucetArgs;
use receive::ReceiveArgs;
use refresh::RefreshArgs;
use reset::ResetArgs;
use send::SendArgs;

pub mod backup;
pub mod balance;
pub mod bridge_in;
pub mod bridge_out;
pub mod change_pwd;
pub mod drain;
pub mod faucet;
pub mod receive;
pub mod refresh;
pub mod reset;
pub mod send;

/// A CLI for interacting with Alpen Labs' devnet signet and rollup.
#[derive(FromArgs, PartialEq, Debug)]
pub struct TopLevel {
    #[argh(subcommand)]
    pub cmd: Commands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Commands {
    Refresh(RefreshArgs),
    Drain(DrainArgs),
    Balance(BalanceArgs),
    Backup(BackupArgs),
    BridgeIn(BridgeInArgs),
    BridgeOut(BridgeOutArgs),
    Faucet(FaucetArgs),
    Send(SendArgs),
    Receive(ReceiveArgs),
    ChangePwd(ChangePwdArgs),
    Reset(ResetArgs),
}
