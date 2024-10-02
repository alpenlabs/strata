use std::{
    fs,
    path::{Path, PathBuf},
};

use alpen_express_primitives::{
    block_credential,
    buf::Buf32,
    operator::OperatorPubkeys,
    params::{RollupParams, SyncParams},
    vk::RollupVerifyingKey,
};
use anyhow::Context;
use argh::FromArgs;
use bech32::{Bech32m, EncodeError, Hrp};
use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    key::Secp256k1,
    params::Params,
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
    #[argh(option, description = "output file path .json (default network name)")]
    output: Option<PathBuf>,

    #[argh(option, description = "network name (default random)", short = 'n')]
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

    #[argh(option, description = "deposit amt in sats (default 10M)")]
    deposit_sats: Option<u64>,

    #[argh(option, description = "genesis trigger height (default 100)")]
    genesis_trigger_height: Option<u64>,

    #[argh(option, description = "SP1 verification key")]
    rollup_vk: Option<String>,

    #[argh(option, description = "block time in millis (default 15k)")]
    block_time_ms: Option<u64>,

    #[argh(option, description = "epoch duration in slots (default 64)")]
    epoch_slots: Option<u32>,
}

pub struct CmdContext {
    /// Resolved datadir for the network.
    datadir: PathBuf,

    /// The network we're using.
    network: Network,

    /// Shared RNG, just `OsRng` for now.
    rng: OsRng,
}

fn main() {
    let args: Args = argh::from_env();

    let mut ctx = CmdContext {
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

fn exec_subc(cmd: Subcommand, ctx: &mut CmdContext) -> anyhow::Result<()> {
    match cmd {
        Subcommand::GenSeed(subc) => exec_genseed(subc, ctx),
        Subcommand::GenSeqPubkey(subc) => exec_genseqpubkey(subc, ctx),
        Subcommand::GenOpXpub(subc) => exec_genopxpub(subc, ctx),
        Subcommand::GenParams(subc) => exec_genparams(subc, ctx),
    }
}

fn exec_genseed(cmd: SubcGenSeed, ctx: &mut CmdContext) -> anyhow::Result<()> {
    if cmd.path.exists() && !cmd.force {
        anyhow::bail!("not overwiting file, add --force to overwrite");
    }

    let xpriv = gen_priv(&mut ctx.rng, ctx.network);
    let buf = xpriv.encode();
    let s = bitcoin::base58::encode_check(&buf);
    fs::write(&cmd.path, s.as_bytes())?;

    Ok(())
}

fn exec_genseqpubkey(cmd: SubcGenSeqPubkey, _ctx: &mut CmdContext) -> anyhow::Result<()> {
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

fn exec_genopxpub(cmd: SubcGenOpXpub, _ctx: &mut CmdContext) -> anyhow::Result<()> {
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

fn exec_genparams(cmd: SubcGenParams, ctx: &mut CmdContext) -> anyhow::Result<()> {
    // TODO update this with vk for checkpoint proof
    let rollup_vk_buf =
        hex::decode("00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f").unwrap();
    let Ok(rollup_vk) = Buf32::try_from(rollup_vk_buf.as_slice()) else {
        anyhow::bail!("malformed verification key");
    };

    // Parse the sequencer key.
    let seqkey = match cmd.seqkey {
        Some(seqkey) => {
            let Ok(buf) = bitcoin::base58::decode_check(&seqkey) else {
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

    let config = ParamsConfig {
        name: cmd.name.unwrap_or_else(|| "strata-testnet".to_string()),
        // TODO make these consts
        block_time_ms: cmd.block_time_ms.unwrap_or(15_000),
        epoch_slots: cmd.epoch_slots.unwrap_or(64),
        genesis_trigger: cmd.genesis_trigger_height.unwrap_or(100),
        seqkey,
        opkeys,
        rollup_vk,
        // TODO make a const
        deposit_sats: cmd.deposit_sats.unwrap_or(1_000_000_000),
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
fn derive_op_purpose_xprivs(master: &Xpriv) -> anyhow::Result<(Xpriv, Xpriv)> {
    let signing_path = DerivationPath::master().extend(&[
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_normal_idx(DERIV_OP_SIGNING_IDX).unwrap(),
    ]);

    let wallet_path = DerivationPath::master().extend(&[
        ChildNumber::from_hardened_idx(DERIV_BASE_IDX).unwrap(),
        ChildNumber::from_hardened_idx(DERIV_OP_IDX).unwrap(),
        ChildNumber::from_normal_idx(DERIV_OP_WALLET_IDX).unwrap(),
    ]);

    let signing_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &signing_path)?;
    let wallet_xpriv = master.derive_priv(bitcoin::secp256k1::SECP256K1, &wallet_path)?;

    Ok((signing_xpriv, wallet_xpriv))
}

/// Derives the signing and wallet xprivs for a Strata operator.
fn derive_op_purpose_xpubs(op_xpub: &Xpub) -> (Xpub, Xpub) {
    let signing_path = DerivationPath::master()
        .extend(&[ChildNumber::from_normal_idx(DERIV_OP_SIGNING_IDX).unwrap()]);

    let wallet_path = DerivationPath::master()
        .extend(&[ChildNumber::from_normal_idx(DERIV_OP_WALLET_IDX).unwrap()]);

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
    block_time_ms: u64,
    epoch_slots: u32,
    genesis_trigger: u64,
    seqkey: Option<Buf32>,
    opkeys: Vec<Xpub>,
    rollup_vk: Buf32,
    deposit_sats: u64,
}

// TODO conver this to also initialize the sync params
fn construct_params(config: ParamsConfig) -> alpen_express_primitives::params::RollupParams {
    let cr = config
        .seqkey
        .map(|k| block_credential::CredRule::SchnorrKey(k))
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

    RollupParams {
        rollup_name: config.name,
        block_time: config.block_time_ms,
        cred_rule: cr,
        horizon_l1_height: config.genesis_trigger / 2,
        genesis_l1_height: config.genesis_trigger,
        operator_config: alpen_express_primitives::params::OperatorConfig::Static(opkeys),
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
        l1_reorg_safe_depth: 4,
        target_l2_batch_size: config.epoch_slots as u64,
        address_length: 20,
        deposit_amount: config.deposit_sats,
        rollup_vk: RollupVerifyingKey::SP1VerifyingKey(config.rollup_vk),
        verify_proofs: true,
        dispatch_assignment_dur: 64,
        proof_publish_mode: alpen_express_primitives::params::ProofPublishMode::Strict,
        max_deposits_in_block: 16,
    }
}

/// Parses an xpub from str, richly generating anyhow results from it.
fn parse_xpub(s: &str) -> anyhow::Result<Xpub> {
    let Ok(buf) = bitcoin::base58::decode_check(s) else {
        anyhow::bail!("failed to parse key: {s}");
    };

    let Ok(xpk) = Xpub::decode(&buf) else {
        anyhow::bail!("failed to decode key: {s}");
    };

    Ok(xpk)
}
