use std::str::FromStr;

use alloy::{primitives::Address as RollupAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use console::Term;
use indicatif::ProgressBar;
use rand::{distributions::uniform::SampleRange, thread_rng};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shrex::{encode, Hex};

use crate::{
    constants::NETWORK, rollup::RollupWallet, seed::Seed, settings::Settings, signet::SignetWallet,
};

/// Request some bitcoin from the faucet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
pub struct FaucetArgs {
    /// request bitcoin on signet
    #[argh(switch)]
    signet: bool,

    /// request bitcoin on rollup
    #[argh(switch)]
    rollup: bool,

    /// address that funds will be sent to. defaults to internal wallet
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

pub async fn faucet(args: FaucetArgs, seed: Seed, settings: Settings) {
    let term = Term::stdout();
    if args.signet && args.rollup {
        let _ = term.write_line("Cannot use both --signet and --rollup options at once");
        std::process::exit(1);
    } else if !args.signet && !args.rollup {
        let _ = term.write_line("Must specify either --signet and --rollup option");
        std::process::exit(1);
    }

    let _ = term.write_line("Fetching challenge from faucet");

    let client = reqwest::Client::new();
    let base = Url::from_str(&settings.faucet_endpoint).expect("valid url");
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
        hasher.update(b"strata faucet 2024");
        hasher.update(challenge.nonce.0);
        hasher
    };
    let pb = ProgressBar::new_spinner();
    while !pow_valid(
        prehash.clone(),
        challenge.difficulty,
        solution.to_le_bytes(),
    ) {
        solution += 1;
        if (0..100).sample_single(&mut thread_rng()) == 0 {
            pb.set_message(format!("Trying {solution}"));
        }
    }
    pb.finish_with_message(format!(
        "✔ Solved challenge after {solution} attempts. Claiming now."
    ));

    let url = if args.signet {
        let mut l1w = SignetWallet::new(&seed, NETWORK).unwrap();
        let address = match args.address {
            None => {
                let address_info = l1w.reveal_next_address(KeychainKind::External);
                l1w.persist().unwrap();
                address_info.address
            }
            Some(address) => {
                let address = Address::from_str(&address).expect("bad address");
                address
                    .require_network(NETWORK)
                    .expect("wrong bitcoin network")
            }
        };

        let _ = term.write_line(&format!("Claiming to signet address {}", address));

        format!(
            "{base}claim_l1/{}/{}",
            encode(&solution.to_le_bytes()),
            address
        )
    } else {
        let l2w = RollupWallet::new(&seed, &settings.l2_http_endpoint).unwrap();
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
    };

    let res = client.get(url).send().await.unwrap();

    let status = res.status();
    let body = res.text().await.expect("invalid response");
    if status == StatusCode::OK {
        let _ = term.write_line(&format!("Successful. Claimed in transaction {body}"));
    } else {
        let _ = term.write_line(&format!("Failed: faucet responded with {status}: {body}"));
    }
}

fn count_leading_zeros(data: &[u8]) -> u8 {
    data.iter()
        .map(|&byte| byte.leading_zeros() as u8)
        .take_while(|&zeros| zeros == 8)
        .sum::<u8>()
}

fn pow_valid(mut hasher: Sha256, difficulty: u8, solution: Solution) -> bool {
    hasher.update(solution);
    count_leading_zeros(&hasher.finalize()) >= difficulty
}
