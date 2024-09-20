use argh::FromArgs;

use crate::seed::change_password;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
/// Changes the seed's encryption password
pub struct ChangePwdArgs {}

pub async fn change_pwd(_args: ChangePwdArgs) {
    change_password().unwrap();
}
