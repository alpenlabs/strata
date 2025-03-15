use argh::FromArgs;
use backup::BackupArgs;
use balance::BalanceArgs;
use change_pwd::ChangePwdArgs;
use config::ConfigArgs;
use drain::DrainArgs;
use faucet::FaucetArgs;
use receive::ReceiveArgs;
use recover::RecoverArgs;
use reset::ResetArgs;
use scan::ScanArgs;
use send::SendArgs;
use withdraw::WithdrawArgs;

pub mod backup;
pub mod balance;
pub mod change_pwd;
pub mod config;
pub mod drain;
pub mod faucet;
pub mod receive;
pub mod recover;
pub mod reset;
pub mod scan;
pub mod send;
pub mod withdraw;

/// A CLI for interacting with Strata and Alpen Labs' bitcoin signet
#[derive(FromArgs, PartialEq, Debug)]
pub struct TopLevel {
    #[argh(subcommand)]
    pub cmd: Commands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Commands {
    Recover(RecoverArgs),
    Drain(DrainArgs),
    Balance(BalanceArgs),
    Backup(BackupArgs),
    Withdraw(WithdrawArgs),
    Faucet(FaucetArgs),
    Send(SendArgs),
    Receive(ReceiveArgs),
    ChangePwd(ChangePwdArgs),
    Reset(ResetArgs),
    Scan(ScanArgs),
    Config(ConfigArgs),
}
