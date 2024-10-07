use std::{
    fs,
    path::{Path, PathBuf},
};

use argh::FromArgs;
use bitcoin::{
    base58,
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    Network,
};
use rand::{rngs::OsRng, Rng};
use strata_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{ProofPublishMode, RollupParams},
    vk::RollupVerifyingKey,
};

// TODO move some of these into a keyderiv crate
const DERIV_BASE_IDX: u32 = 56;
const DERIV_SEQ_IDX: u32 = 10;
const DERIV_OP_IDX: u32 = 20;
const DERIV_OP_SIGNING_IDX: u32 = 100;
const DERIV_OP_WALLET_IDX: u32 = 101;
const SEQKEY_ENVVAR: &str = "STRATA_SEQ_KEY";
const OPKEY_ENVVAR: &str = "STRATA_OP_KEY";
const DEFAULT_NETWORK: Network = Network::Signet;

/// List of keys that are used in examples that we don't want people to actually
/// use.  If any of these are present in a command invocation we abort immediately.
const KEY_BLACKLIST: &[&str] = &[
    "XGUgTAJNpexzrjgnbMvGtDBCZEwxd6KQE4PNDWE6YLZYBTGoS",
    "tpubDASVk1m5cxpmUbwVEZEQb8maDVx9kDxBhSLCqsKHJJmZ8htSegpHx7G3RFudZCdDLtNKTosQiBLbbFsVA45MemurWenzn16Y1ft7NkQekcD",
    "tpubDBX9KQsqK2LMCszkDHvANftHzhJdhipe9bi9MNUD3S2bsY1ikWEZxE53VBgYN8WoNXk9g9eRzhx6UfJcQr3XqkA27aSxXvKu5TYFZJEAjCd"
];

/// Args.
#[derive(FromArgs)]
pub struct Args {
    #[argh(option, description = "network name [signet, regtest]", short = 'b')]
    bitcoin_network: Option<String>,

    #[argh(
        option,
        description = "data directory (unused) (default cwd)",
        short = 'd'
    )]
    datadir: Option<PathBuf>,

    #[argh(subcommand)]
    subc: Subcommand,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    GenSeed(SubcGenSeed),
    GenSeqPubkey(SubcGenSeqPubkey),
    GenSeqPrivkey(SubcGenSeqPrivkey),
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

/// Generate the sequencer pubkey to pass around.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "genseqprivkey",
    description = "generates a sequencer privkey from seed"
)]
pub struct SubcGenSeqPrivkey {
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
    #[argh(option, description = "output file path .json (default stdout)")]
    output: Option<PathBuf>,

    #[argh(
        option,
        description = "network name, used for magics (default random)",
        short = 'n'
    )]
    name: Option<String>,

    #[argh(
        option,
        description = "sequencer pubkey (default unchecked)",
        short = 's'
    )]
    seqkey: Option<String>,

    #[argh(
        option,
        description = "add a bridge operator key (must be at least one, appended after file keys)",
        short = 'b'
    )]
    opkey: Vec<String>,

    #[argh(
        option,
        description = "read bridge operator keys by line from file",
        short = 'B'
    )]
    opkeys: Option<PathBuf>,

    #[argh(option, description = "deposit amount in sats (default \"10 BTC\")")]
    deposit_sats: Option<String>,

    #[argh(
        option,
        description = "genesis trigger height (default 100)",
        short = 'g'
    )]
    genesis_trigger_height: Option<u64>,

    #[argh(option, description = "SP1 verification key")]
    rollup_vk: Option<String>,

    #[argh(
        option,
        description = "block time in seconds (default 15)",
        short = 't'
    )]
    block_time: Option<u64>,

    #[argh(option, description = "epoch duration in slots (default 64)")]
    epoch_slots: Option<u32>,

    #[argh(
        option,
        description = "permit blank proofs after timeout in millis (default strict)"
    )]
    proof_timeout: Option<u32>,
}

pub struct CmdContext {
    /// Resolved datadir for the network.
    #[allow(unused)]
    datadir: PathBuf,

    /// The Bitcoin network we're building on top of.
    bitcoin_network: Network,

    /// Shared RNG, just [`OsRng`] for now.
    rng: OsRng,
}

fn main() {
    let args: Args = argh::from_env();
    if let Err(e) = main_inner(args) {
        eprintln!("{e}\n{e:?}");
    }
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    let network = resolve_network(args.bitcoin_network.as_deref())?;

    let mut ctx = CmdContext {
        datadir: args.datadir.unwrap_or_else(|| PathBuf::from(".")),
        bitcoin_network: network,
        rng: OsRng,
    };

    exec_subc(args.subc, &mut ctx)?;
    Ok(())
}

fn resolve_network(arg: Option<&str>) -> anyhow::Result<Network> {
    match arg {
        Some("signet") => Ok(Network::Signet),
        Some("regtest") => Ok(Network::Regtest),
        Some(n) => anyhow::bail!("unsupported network option: {n}"),
        None => Ok(DEFAULT_NETWORK),
    }
}

fn exec_subc(cmd: Subcommand, ctx: &mut CmdContext) -> anyhow::Result<()> {
    match cmd {
        Subcommand::GenSeed(subc) => exec_genseed(subc, ctx),
        Subcommand::GenSeqPubkey(subc) => exec_genseqpubkey(subc, ctx),
        Subcommand::GenSeqPrivkey(subc) => exec_genseqprivkey(subc, ctx),
        Subcommand::GenOpXpub(subc) => exec_genopxpub(subc, ctx),
        Subcommand::GenParams(subc) => exec_genparams(subc, ctx),
    }
}

fn exec_genseed(cmd: SubcGenSeed, ctx: &mut CmdContext) -> anyhow::Result<()> {
    if cmd.path.exists() && !cmd.force {
        anyhow::bail!("not overwriting file, add --force to overwrite");
    }

    let xpriv = gen_priv(&mut ctx.rng, ctx.bitcoin_network);
    let buf = xpriv.encode();
    let s = base58::encode_check(&buf);
    fs::write(&cmd.path, s.as_bytes())?;

    Ok(())
}

fn exec_genseqpubkey(cmd: SubcGenSeqPubkey, _ctx: &mut CmdContext) -> anyhow::Result<()> {
    let Some(xpriv) = resolve_xpriv(&cmd.key_file, cmd.key_from_env, SEQKEY_ENVVAR)? else {
        anyhow::bail!("privkey unset");
    };

    let seq_xpriv = derive_seq_xpriv(&xpriv)?;
    let seq_xpub = Xpub::from_priv(bitcoin::secp256k1::SECP256K1, &seq_xpriv);
    let raw_buf = seq_xpub.to_x_only_pub().serialize();
    let s = base58::encode_check(&raw_buf);

    println!("{s}");

    Ok(())
}

fn exec_genseqprivkey(cmd: SubcGenSeqPrivkey, _ctx: &mut CmdContext) -> anyhow::Result<()> {
    let Some(xpriv) = resolve_xpriv(&cmd.key_file, cmd.key_from_env, SEQKEY_ENVVAR)? else {
        anyhow::bail!("privkey unset");
    };

    let seq_xpriv = derive_seq_xpriv(&xpriv)?;
    let raw_buf = seq_xpriv.to_priv().to_bytes();
    let s = base58::encode_check(&raw_buf);

    println!("{s}");

    Ok(())
}

fn exec_genopxpub(cmd: SubcGenOpXpub, _ctx: &mut CmdContext) -> anyhow::Result<()> {
    let Some(xpriv) = resolve_xpriv(&cmd.key_file, cmd.key_from_env, OPKEY_ENVVAR)? else {
        anyhow::bail!("privkey unset");
    };

    let op_xpriv = derive_op_root_xpub(&xpriv)?;
    let op_xpub = Xpub::from_priv(bitcoin::secp256k1::SECP256K1, &op_xpriv);
    let raw_buf = op_xpub.encode();
    let s = base58::encode_check(&raw_buf);

    println!("{s}");

    Ok(())
}

fn exec_genparams(cmd: SubcGenParams, ctx: &mut CmdContext) -> anyhow::Result<()> {
    // Parse the sequencer key.
    let seqkey = match cmd.seqkey {
        Some(seqkey) => {
            let Ok(buf) = base58::decode_check(&seqkey) else {
                anyhow::bail!("failed to parse sequencer key: {seqkey}");
            };

            let Ok(buf) = Buf32::try_from(buf.as_slice()) else {
                anyhow::bail!("invalid sequencer key (must be 32 bytes): {seqkey}");
            };

            Some(buf)
        }
        None => None,
    };

    // Parse each of the operator keys.
    let mut opkeys = Vec::new();

    if let Some(opkeys_path) = cmd.opkeys {
        let opkeys_str = fs::read_to_string(opkeys_path)?;

        for l in opkeys_str.lines() {
            // skip lines that are empty or look like comments
            if l.trim().is_empty() || l.starts_with("#") {
                continue;
            }

            opkeys.push(parse_xpub(l)?);
        }
    }

    for k in cmd.opkey {
        opkeys.push(parse_xpub(&k)?);
    }

    // Parse the deposit size str.
    let deposit_sats = cmd
        .deposit_sats
        .map(|s| parse_abbr_amt(&s))
        .transpose()?
        .unwrap_or(1_000_000_000);

    // Parse the checkpoint verification key.
    let rollup_vk = {
        let vk_buf = match cmd.rollup_vk {
            Some(s) => hex::decode(s)?,

            // TODO update this with vk for checkpoint proof
            None => hex::decode("00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f")
                .unwrap(),
        };

        let Ok(vk) = Buf32::try_from(vk_buf.as_slice()) else {
            anyhow::bail!("malformed verification key");
        };

        vk
    };

    let config = ParamsConfig {
        name: cmd.name.unwrap_or_else(|| "strata-testnet".to_string()),
        bitcoin_network: ctx.bitcoin_network,
        // TODO make these consts
        block_time_sec: cmd.block_time.unwrap_or(15),
        epoch_slots: cmd.epoch_slots.unwrap_or(64),
        genesis_trigger: cmd.genesis_trigger_height.unwrap_or(100),
        seqkey,
        opkeys,
        rollup_vk,
        // TODO make a const
        deposit_sats,
        proof_timeout: cmd.proof_timeout,
    };

    let params = construct_params(config);
    let params_buf = serde_json::to_string_pretty(&params)?;

    if let Some(out_path) = &cmd.output {
        fs::write(out_path, params_buf)?;
        eprintln!("wrote to file {out_path:?}");
    } else {
        println!("{params_buf}");
    }

    Ok(())
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
    let buf = base58::decode_check(str_buf)?;
    Ok(Xpriv::decode(&buf)?)
}

/// Resolves a key from set vars and whatnot.
fn resolve_xpriv(
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

            let buf = base58::decode_check(&val)?;
            Ok(Some(Xpriv::decode(&buf)?))
        }
        _ => Ok(None),
    }
}

fn derive_strata_scheme_xpriv(master: &Xpriv, last: u32) -> anyhow::Result<Xpriv> {
    let derivation_path = DerivationPath::master().extend([
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
fn derive_op_purpose_xpubs(op_xpub: &Xpub) -> (Xpub, Xpub) {
    let signing_path = DerivationPath::master()
        .extend([ChildNumber::from_normal_idx(DERIV_OP_SIGNING_IDX).unwrap()]);

    let wallet_path = DerivationPath::master()
        .extend([ChildNumber::from_normal_idx(DERIV_OP_WALLET_IDX).unwrap()]);

    let signing_xpub = op_xpub
        .derive_pub(bitcoin::secp256k1::SECP256K1, &signing_path)
        .unwrap();
    let wallet_xpub = op_xpub
        .derive_pub(bitcoin::secp256k1::SECP256K1, &wallet_path)
        .unwrap();

    (signing_xpub, wallet_xpub)
}

/// Describes inputs for how we want to set params.
pub struct ParamsConfig {
    name: String,
    #[allow(unused)]
    bitcoin_network: Network,
    block_time_sec: u64,
    epoch_slots: u32,
    genesis_trigger: u64,
    seqkey: Option<Buf32>,
    opkeys: Vec<Xpub>,
    rollup_vk: Buf32,
    deposit_sats: u64,
    proof_timeout: Option<u32>,
}

// TODO convert this to also initialize the sync params
fn construct_params(config: ParamsConfig) -> strata_primitives::params::RollupParams {
    let cr = config
        .seqkey
        .map(block_credential::CredRule::SchnorrKey)
        .unwrap_or(block_credential::CredRule::Unchecked);

    let opkeys = config
        .opkeys
        .into_iter()
        .map(|xpk| {
            let (signing_key, wallet_key) = derive_op_purpose_xpubs(&xpk);
            let signing_key_buf = signing_key.to_x_only_pub().serialize().into();
            let wallet_key_buf = wallet_key.to_x_only_pub().serialize().into();
            OperatorPubkeys::new(signing_key_buf, wallet_key_buf)
        })
        .collect::<Vec<_>>();

    // TODO add in bitcoin network

    RollupParams {
        rollup_name: config.name,
        block_time: config.block_time_sec * 1000,
        cred_rule: cr,
        // TODO do we want to remove this?
        horizon_l1_height: config.genesis_trigger / 2,
        genesis_l1_height: config.genesis_trigger,
        operator_config: strata_primitives::params::OperatorConfig::Static(opkeys),
        // TODO make configurable
        evm_genesis_block_hash: Buf32(
            "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                .parse()
                .unwrap(),
        ),
        evm_genesis_block_state_root: Buf32(
            "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                .parse()
                .unwrap(),
        ),
        // TODO make configurable
        l1_reorg_safe_depth: 4,
        target_l2_batch_size: config.epoch_slots as u64,
        address_length: 20,
        deposit_amount: config.deposit_sats,
        rollup_vk: RollupVerifyingKey::SP1VerifyingKey(config.rollup_vk),
        // TODO make configurable
        dispatch_assignment_dur: 64,
        proof_publish_mode: config
            .proof_timeout
            .map(|t| ProofPublishMode::Timeout(t as u64))
            .unwrap_or(ProofPublishMode::Strict),
        // TODO make configurable
        max_deposits_in_block: 16,
        network: config.bitcoin_network,
    }
}

/// Returns an `Err` if the provided key we're trying to parse is on the blacklist.
fn check_key_not_blacklisted(s: &str) -> anyhow::Result<()> {
    let ts = s.trim();
    if KEY_BLACKLIST.contains(&ts) {
        anyhow::bail!("that was an example!  generate your own keys!");
    }

    Ok(())
}

/// Parses an [`Xpub`] from [`&str`], richly generating [`anyhow::Result`]s from
/// it.
fn parse_xpub(s: &str) -> anyhow::Result<Xpub> {
    check_key_not_blacklisted(s)?;

    let Ok(buf) = base58::decode_check(s) else {
        anyhow::bail!("failed to parse key: {s}");
    };

    let Ok(xpk) = Xpub::decode(&buf) else {
        anyhow::bail!("failed to decode key: {s}");
    };

    Ok(xpk)
}

fn parse_abbr_amt(s: &str) -> anyhow::Result<u64> {
    // Thousand.
    if let Some(v) = s.strip_suffix("K") {
        return Ok(v.parse::<u64>()? * 1000);
    }

    // Million.
    if let Some(v) = s.strip_suffix("M") {
        return Ok(v.parse::<u64>()? * 1_000_000);
    }

    // Billion.
    if let Some(v) = s.strip_suffix("G") {
        return Ok(v.parse::<u64>()? * 1_000_000_000);
    }

    // Trillion, probably not necessary.
    if let Some(v) = s.strip_suffix("T") {
        return Ok(v.parse::<u64>()? * 1_000_000_000_000);
    }

    // Simple value.
    Ok(s.parse::<u64>()?)
}
