use argh::FromArgs;

use crate::{recovery::DescriptorRecovery, rollup::RollupWallet, seed::Seed, signet::SignetWallet};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "refresh")]
/// Runs any background tasks manually
pub struct RefreshArgs {}

pub async fn refresh(seed: Option<Seed>) {
    let seed = seed.unwrap_or_else(|| Seed::load_or_create().unwrap());
    let l1w = SignetWallet::new(&seed).unwrap();
    let l2w = RollupWallet::new(&seed).unwrap();

    let mut descriptor_file = DescriptorRecovery::open(&seed).await.unwrap();
    // let descs = descriptor_file.read_descs().await.unwrap();
}
