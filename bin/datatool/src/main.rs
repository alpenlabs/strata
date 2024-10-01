use std::path::PathBuf;

use argh::FromArgs;
use bech32::{Bech32m, EncodeError, Hrp};
use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    key::Secp256k1,
    secp256k1::All,
    Network,
};
use rand::{rngs::OsRng, thread_rng, Rng};

const NETWORK: Network = Network::Signet;

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
    GenSeqPubkey(SubcGenSeqPubkey),
    GenOpXpub(SubcGenOpXpub),
    GenParams(SubcGenParams),
}

/// Generate the sequencer pubkey to pass around.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "genseqpubkey")]
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
#[argh(subcommand, name = "genopxpub")]
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
#[argh(subcommand, name = "genparams")]
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
        panic!("{e} {e:?}")
    }

    let secp = Secp256k1::new();
    let master_priv = gen_priv(&mut thread_rng());

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
}

fn resolve_network(arg: Option<&str>) -> Network {
    match arg {
        Some("signet") => Network::Signet,
        Some("regtest") => Network::Regtest,
        Some(n) => panic!("unsupported network option: {n}"),
        None => NETWORK,
    }
}

fn exec_subc(cmd: Subcommand, ctx: &mut Context) -> anyhow::Result<()> {
    match cmd {
        Subcommand::GenSeqPubkey(subc) => {}
        Subcommand::GenOpXpub(subc) => {}
        Subcommand::GenParams(subc) => {}
    }
})

fn exec_genseqpubkey(cmd: SubcGenSeqPubkey, ctx: &mut Context) -> anyhow::Result<()> {
    unimplemented!()
}

fn exec_genopxpub(cmd: SubcGenOpXpub, ctx: &mut Context) -> anyhow::Result<()> {
    unimplemented!()
}

fn exec_genparams(cmd: SubcGenParams, ctx: &mut Context) -> anyhow::Result<()> {
    unimplemented!()
}

fn gen_priv(rng: &mut impl Rng) -> Xpriv {
    let seed: [u8; 32] = rng.gen();
    Xpriv::new_master(NETWORK, &seed).expect("valid seed")
}

enum Key {
    Public(Xpub),
    Private(Xpriv),
}

impl Key {
    fn decode(value: String) -> Option<Key> {
        let (hrp, data) = bech32::decode(&value).ok()?;
        if hrp == Xpriv::HRP {
            let master = Xpriv::decode(&data).ok()?;
            Some(Key::Private(master))
        } else if hrp == Xpub::HRP {
            let public = Xpub::decode(&data).ok()?;
            Some(Key::Public(public))
        } else {
            None
        }
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Key::Public(xpub) => xpub.strata_encode(),
            Key::Private(xpriv) => xpriv.strata_encode(),
        }
        .expect("successful encode");
        f.write_str(&value)
    }
}

struct Keys {
    operator: Key,
    sequencer: Key,
}

impl Keys {
    fn derive(master: Key, secp: &Secp256k1<All>) -> Self {
        let derivation_path = DerivationPath::master().extend(&[
            ChildNumber::from_hardened_idx(86).expect("valid child number"),
            ChildNumber::from_hardened_idx(0).expect("valid child number"),
            ChildNumber::from_hardened_idx(0).expect("valid child number"),
            ChildNumber::from_normal_idx(0).expect("valid child number"),
        ]);
        let operator_path =
            derivation_path.extend(ChildNumber::from_normal_idx(0).expect("valid child number"));
        let sequencer_path =
            derivation_path.extend(ChildNumber::from_normal_idx(1).expect("valid child number"));
        match master {
            Key::Public(xpub) => {
                let operator_pubk = xpub.derive_pub(&secp, &operator_path).expect("valid path");
                let sequencer_pubk = xpub.derive_pub(&secp, &sequencer_path).expect("valid path");
                Keys {
                    operator: Key::Public(operator_pubk),
                    sequencer: Key::Public(sequencer_pubk),
                }
            }
            Key::Private(xpriv) => {
                let operator_privk = xpriv
                    .derive_priv(&secp, &operator_path)
                    .expect("valid path");
                let sequencer_privk = xpriv
                    .derive_priv(&secp, &sequencer_path)
                    .expect("valid path");
                Keys {
                    operator: Key::Private(operator_privk),
                    sequencer: Key::Private(sequencer_privk),
                }
            }
        }
    }
}

trait StrataKeyEncodable {
    const HRP: Hrp;

    fn as_bytes(&self) -> [u8; 78];

    fn strata_encode(&self) -> Result<String, EncodeError> {
        bech32::encode::<Bech32m>(Self::HRP, &self.as_bytes())
    }
}

impl StrataKeyEncodable for Xpriv {
    const HRP: Hrp = Hrp::parse_unchecked("strata_sec");

    fn as_bytes(&self) -> [u8; 78] {
        self.encode()
    }
}

impl StrataKeyEncodable for Xpub {
    const HRP: Hrp = Hrp::parse_unchecked("strata_pub");

    fn as_bytes(&self) -> [u8; 78] {
        self.encode()
    }
}
