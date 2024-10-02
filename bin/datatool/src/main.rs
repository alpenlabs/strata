use std::{
    fs,
    path::{Path, PathBuf},
};

use argh::FromArgs;
use bech32::{Bech32m, EncodeError, Hrp};
use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    key::Secp256k1,
    secp256k1::All,
    Network,
};
use rand::{rngs::OsRng, thread_rng, Rng};

const DERIV_BASE_IDX: u32 = 56;
const DERIV_SEQ_IDX: u32 = 10;
const DERIV_OP_IDX: u32 = 20;
const DERIV_OP_SIGNING_IDX: u32 = 100;
const DERIV_OP_WALLET_IDX: u32 = 101;
const SEQKEY_ENVVAR: &str = "STRATA_SEQ_KEY";
const OPKEY_ENVVAR: &str = "STRATA_OP_KEY";
const DEFAULT_NETWORK: Network = Network::Signet;

/// Args.
#[derive(FromArgs)]
pub struct Args {
    #[argh(option, description = "network name [signet, regtest]", short = 'b')]
    bitcoin_network: Option<String>,

    #[argh(subcommand)]
    subc: Subcommand,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    GenSeed(SubcGenSeed),
    GenSeqPubkey(SubcGenSeqPubkey),
    GenOpXpub(SubcGenOpXpub),
    GenParams(SubcGenParams),
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "genseed",
    description = "generates a xpriv and writes it to a file"
)]
pub struct SubcGenSeed {
    #[argh(positional, description = "output path")]
    path: PathBuf,

    #[argh(switch, description = "force overwrite", short = 'f')]
    force: bool,
}

/// Generate the sequencer pubkey to pass around.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "genseqpubkey",
    description = "generates a sequencer pubkey from seed"
)]
pub struct SubcGenSeqPubkey {
    #[argh(option, description = "reads key from specified file", short = 'f')]
    key_file: Option<PathBuf>,

    #[argh(
        switch,
        description = "reads key from envvar STRATA_SEQ_KEY",
        short = 'E'
    )]
    key_from_env: bool,
}

/// Generate operator xpub to pass around.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "genopxpub",
    description = "generates an operator xpub from seed"
)]
pub struct SubcGenOpXpub {
    #[argh(option, description = "reads key from specified file", short = 'f')]
    key_file: Option<PathBuf>,

    #[argh(
        switch,
        description = "reads key from envvar STRATA_OP_KEY",
        short = 'E'
    )]
    key_from_env: bool,
}

/// Generate a network's param file from inputs.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "genparams",
    description = "generates network params from inputs"
)]
pub struct SubcGenParams {
    #[argh(option, description = "network name (default random)", short = 'n')]
    name: Option<String>,

    #[argh(option, description = "output file path .json (default network name)")]
    output: Option<PathBuf>,

    #[argh(option, description = "sequencer pubkey")]
    seqkey: Option<String>,

    #[argh(option, description = "add an operator key (must be at least one)")]
    opkey: Vec<String>,

    #[argh(option, description = "read operator keys by line from file")]
    opkeys: Option<PathBuf>,
}

pub struct Context {
    /// Resolved datadir for the network.
    datadir: PathBuf,

    /// The network we're using.
    network: Network,

    /// Shared RNG, just `OsRng` for now.
    rng: OsRng,
}

fn main() {
    let args: Args = argh::from_env();

    let mut ctx = Context {
        datadir: PathBuf::from("."),
        network: resolve_network(args.bitcoin_network.as_ref().map(|s| s.as_str())),
        rng: OsRng,
    };

    if let Err(e) = exec_subc(args.subc, &mut ctx) {
        eprintln!("{e}\n{e:?}");
        return;
    }

    /*let secp = Secp256k1::new();
    let master_priv = gen_priv(&mut thread_rng(), DEFAULT_NETWORK);
    println!(
        "Private: {}",
        master_priv.strata_encode().expect("successful encode")
    );
    let master_pub = Xpub::from_priv(&secp, &master_priv);
    println!(
        "Public: {}",
        master_pub.strata_encode().expect("successful encode")
    );

    let keys = Keys::derive(Key::Private(master_priv), &secp);
    println!("sequencer key: {}", keys.sequencer);
    println!("operator key: {}", keys.operator);
     */
}

fn resolve_network(arg: Option<&str>) -> Network {
    match arg {
        Some("signet") => Network::Signet,
        Some("regtest") => Network::Regtest,
        Some(n) => panic!("unsupported network option: {n}"),
        None => DEFAULT_NETWORK,
    }
}

fn exec_subc(cmd: Subcommand, ctx: &mut Context) -> anyhow::Result<()> {
    match cmd {
        Subcommand::GenSeed(subc) => exec_genseed(subc, ctx),
        Subcommand::GenSeqPubkey(subc) => exec_genseqpubkey(subc, ctx),
        Subcommand::GenOpXpub(subc) => exec_genopxpub(subc, ctx),
        Subcommand::GenParams(subc) => exec_genparams(subc, ctx),
    }
}

fn exec_genseed(cmd: SubcGenSeed, ctx: &mut Context) -> anyhow::Result<()> {
    if cmd.path.exists() && !cmd.force {
        anyhow::bail!("not overwiting file, add --force to overwrite");
    }

    let xpriv = gen_priv(&mut ctx.rng, ctx.network);
    let buf = xpriv.encode();
    let s = bitcoin::base58::encode_check(&buf);
    fs::write(&cmd.path, s.as_bytes())?;

    Ok(())
}

fn exec_genseqpubkey(cmd: SubcGenSeqPubkey, _ctx: &mut Context) -> anyhow::Result<()> {
    let Some(xpriv) = resolve_key(&cmd.key_file, cmd.key_from_env, &SEQKEY_ENVVAR)? else {
        anyhow::bail!("privkey unset");
    };

    let seq_xpriv = derive_seq_xpriv(&xpriv)?;
    let seq_xpub = Xpub::from_priv(bitcoin::secp256k1::SECP256K1, &seq_xpriv);
    let raw_buf = seq_xpub.to_x_only_pub().serialize();
    let s = bitcoin::base58::encode_check(&raw_buf);

    eprintln!("{s}");

    Ok(())
}

fn exec_genopxpub(cmd: SubcGenOpXpub, _ctx: &mut Context) -> anyhow::Result<()> {
    let Some(xpriv) = resolve_key(&cmd.key_file, cmd.key_from_env, &OPKEY_ENVVAR)? else {
        anyhow::bail!("privkey unset");
    };

    let op_xpriv = derive_op_root_xpub(&xpriv)?;
    let op_xpub = Xpub::from_priv(bitcoin::secp256k1::SECP256K1, &op_xpriv);
    let raw_buf = op_xpub.encode();
    let s = bitcoin::base58::encode_check(&raw_buf);

    eprintln!("{s}");

    Ok(())
}

fn exec_genparams(cmd: SubcGenParams, ctx: &mut Context) -> anyhow::Result<()> {
    unimplemented!()
}

/// Generates a new xpriv.
fn gen_priv(rng: &mut impl Rng, net: Network) -> Xpriv {
    let seed: [u8; 32] = rng.gen();
    Xpriv::new_master(net, &seed).expect("valid seed")
}

/// Reads an xprv from file as a string, verifying the checksom.
fn read_xpriv(path: &Path) -> anyhow::Result<Xpriv> {
    let raw_buf = fs::read(path)?;
    let str_buf = std::str::from_utf8(&raw_buf)?;
    let buf = bitcoin::base58::decode_check(str_buf)?;
    Ok(Xpriv::decode(&buf)?)
}

/// Resolves a key from set vars and whatnot.
fn resolve_key(
    path: &Option<PathBuf>,
    from_env: bool,
    env: &'static str,
) -> anyhow::Result<Option<Xpriv>> {
    match (path, from_env) {
        (Some(_), true) => anyhow::bail!("got key path and --key-from-env, pick a lane"),
        (Some(path), false) => Ok(Some(read_xpriv(path)?)),
        (None, true) => {
            let Ok(val) = std::env::var(env) else {
                anyhow::bail!("got --key-from-env but {env} not set or invalid");
            };

            let buf = bitcoin::base58::decode_check(&val)?;
            Ok(Some(Xpriv::decode(&buf)?))
        }
        _ => Ok(None),
    }
}

fn derive_strata_scheme_xpriv(master: &Xpriv, last: u32) -> anyhow::Result<Xpriv> {
    let derivation_path = DerivationPath::master().extend(&[
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(last).unwrap(),
    ]);
    Ok(master.derive_priv(bitcoin::secp256k1::SECP256K1, &derivation_path)?)
}

/// Derives the sequencer xpriv.
fn derive_seq_xpriv(master: &Xpriv) -> anyhow::Result<Xpriv> {
    derive_strata_scheme_xpriv(master, DERIV_SEQ_IDX)
}

/// Derives the root xpub for a Strata operator which can be turned into an xpub
/// and used in network init.
fn derive_op_root_xpub(master: &Xpriv) -> anyhow::Result<Xpriv> {
    derive_strata_scheme_xpriv(master, DERIV_OP_IDX)
}

/// Derives the signing and wallet xprivs for a Strata operator.
fn derive_op_signing_xpriv(master: &Xpriv) -> anyhow::Result<(Xpriv, Xpriv)> {
    let signing_path = DerivationPath::master().extend(&[
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_SIGNING_IDX).unwrap(),
    ]);

    let wallet_path = DerivationPath::master().extend(&[
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_WALLET_IDX).unwrap(),
    ]);

    let signing_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &signing_path)?;
    let wallet_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &wallet_path)?;

    Ok((signing_xpriv, wallet_xpriv))
}
