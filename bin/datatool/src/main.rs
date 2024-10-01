use bech32::{Bech32m, EncodeError, Hrp};
use bitcoin::{
    bip32::{ChildNumber, DerivationPath, Xpriv, Xpub},
    key::Secp256k1,
    secp256k1::All,
    Network,
};
use rand::{thread_rng, Rng};

const NETWORK: Network = Network::Signet;

fn main() {
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
