use std::path::PathBuf;

use argh::FromArgs;

mod fix_checkpoint_key_encoding;

/// Args.
#[derive(FromArgs)]
pub struct Args {
    #[argh(option, description = "rocksdb datadir path")]
    datadir: PathBuf,
    #[argh(switch, description = "commit changes to db")]
    commit: bool,
    #[argh(subcommand)]
    subc: Subcommand,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    FixCheckpointKeyEncoding(SubcFixCheckpointKeyEncoding),
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "fix-checkpoint-key-encoding",
    description = "fix checkpoint key encoding"
)]
pub struct SubcFixCheckpointKeyEncoding {}

fn main() {
    let args: Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("{e}\n{e:?}");
    }
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    match args.subc {
        Subcommand::FixCheckpointKeyEncoding(_) => {
            fix_checkpoint_key_encoding::db_upgrade(args.datadir, args.commit)?
        }
    };

    Ok(())
}
