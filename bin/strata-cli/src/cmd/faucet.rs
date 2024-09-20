use std::str::FromStr;

use alloy::{primitives::Address as RollupAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, rusqlite::Connection, KeychainKind};
use console::Term;
use hex::{encode, Hex};
use indicatif::ProgressBar;
use rand::{distributions::uniform::SampleRange, thread_rng};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{rollup::RollupWallet, seed::Seed, settings::SETTINGS, signet::SignetWallet};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
/// Request some bitcoin from the faucet
pub struct FaucetArgs {
    #[argh(switch)]
    /// request signet bitcoin
    signet: bool,
    #[argh(switch)]
    /// request rollup bitcoin
    rollup: bool,
    #[argh(positional)]
    address: Option<String>,
}

type Nonce = [u8; 16];
type Solution = [u8; 8];

#[derive(Debug, Serialize, Deserialize)]
pub struct PowChallenge {
    nonce: Hex<Nonce>,
    difficulty: u8,
}

pub async fn faucet(args: FaucetArgs) {
    let term = Term::stdout();
    if args.signet && args.rollup {
        let _ = term.write_line("Cannot use both --signet and --rollup options at once");
        std::process::exit(1);
    } else if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    let seed = Seed::load_or_create().unwrap();

    let _ = term.write_line("Fetching challenge from faucet");

    let client = reqwest::Client::new();
    let base = Url::from_str(&SETTINGS.faucet_endpoint).expect("valid url");
    let challenge = client
        .get(base.join("/pow_challenge").unwrap())
        .send()
        .await
        .unwrap()
        .json::<PowChallenge>()
        .await
        .expect("invalid response");
    let _ = term.write_line(&format!(
        "Received POW challenge with difficulty 2^{} from faucet: {:?}. Solving...",
        challenge.difficulty, challenge.nonce
    ));

    let mut solution = 0u64;
    let prehash = {
        let mut hasher = Sha256::new();
        hasher.update(b"alpen labs faucet 2024");
        hasher.update(challenge.nonce.0);
        hasher
    };
    let pb = ProgressBar::new_spinner();
    let mut rng = thread_rng();
    while !pow_valid(
        prehash.clone(),
        challenge.difficulty,
        solution.to_le_bytes(),
    ) {
        solution += 1;
        if (0..100).sample_single(&mut rng) == 0 {
            pb.set_message(format!("Trying {solution}"));
        }
    }
    pb.finish_with_message(format!(
        "✔ Solved challenge after {} attempts. Claiming now.",
        solution
    ));

    let url = if args.signet {
        let mut conn = SignetWallet::persister().unwrap();
        let mut l1w = SignetWallet::new(seed.signet_wallet()).unwrap();
        let address = match args.address {
            None => {
                let address_info = l1w.reveal_next_address(KeychainKind::External);
                l1w.persist(&mut conn).unwrap();
                address_info.address
            }
            Some(address) => {
                let address = Address::from_str(&address).expect("bad address");
                address
                    .require_network(SETTINGS.network)
                    .expect("wrong bitcoin network")
            }
        };

        let _ = term.write_line(&format!("Claiming to signet address {}", address));

        format!(
            "{base}claim_l1/{}/{}",
            encode(&solution.to_le_bytes()),
            address
        )
    } else if args.rollup {
        let l2w = RollupWallet::new(&seed).unwrap();
        // they said EVMs were advanced 👁️👁️
        let address = match args.address {
            Some(address) => RollupAddress::from_str(&address).expect("bad address"),
            None => l2w.default_signer_address(),
        };
        let _ = term.write_line(&format!("Claiming to rollup address {}", address));
        format!(
            "{base}claim_l2/{}/{}",
            encode(&solution.to_le_bytes()),
            address
        )
    } else {
        unreachable!()
    };

    let res = client.get(url).send().await.unwrap();

    let status = res.status();
    let body = res.text().await.expect("invalid response");
    if status == StatusCode::OK {
        let _ = term.write_line(&format!("Successful. Claimed in transaction {}", body));
    } else {
        let _ = term.write_line(&format!(
            "Failed: faucet responded with {}: {}",
            status, body
        ));
    }
}

fn count_leading_zeros(data: &[u8]) -> u8 {
    let mut leading_zeros = 0;
    for byte in data {
        if *byte == 0 {
            leading_zeros += 8;
        } else {
            leading_zeros += byte.leading_zeros() as u8;
            break;
        }
    }

    leading_zeros
}

fn pow_valid(mut hasher: Sha256, difficulty: u8, solution: Solution) -> bool {
    hasher.update(solution);
    count_leading_zeros(&hasher.finalize()) >= difficulty
}
