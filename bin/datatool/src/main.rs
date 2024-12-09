mod args;
mod utils;

use std::path::PathBuf;

use args::CmdContext;
use rand::rngs::OsRng;
use utils::{exec_subc, resolve_network};

/// List of keys that are used in examples that we don't want people to actually
/// use.
///
/// If any of these are present in a command invocation we abort immediately.
const KEY_BLACKLIST: &[&str] = &[
    "XGUgTAJNpexzrjgnbMvGtDBCZEwxd6KQE4PNDWE6YLZYBTGoS",
    "tpubDASVk1m5cxpmUbwVEZEQb8maDVx9kDxBhSLCqsKHJJmZ8htSegpHx7G3RFudZCdDLtNKTosQiBLbbFsVA45MemurWenzn16Y1ft7NkQekcD",
    "tpubDBX9KQsqK2LMCszkDHvANftHzhJdhipe9bi9MNUD3S2bsY1ikWEZxE53VBgYN8WoNXk9g9eRzhx6UfJcQr3XqkA27aSxXvKu5TYFZJEAjCd"
];

fn main() {
    let args: args::Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("ERROR\n{e:?}");
    }
}

fn main_inner(args: args::Args) -> anyhow::Result<()> {
    let network = resolve_network(args.bitcoin_network.as_deref())?;

    let mut ctx = CmdContext {
        datadir: args.datadir.unwrap_or_else(|| PathBuf::from(".")),
        bitcoin_network: network,
        rng: OsRng,
    };

    exec_subc(args.subc, &mut ctx)?;
    Ok(())
}
