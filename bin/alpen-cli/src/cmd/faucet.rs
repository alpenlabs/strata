use std::str::FromStr;

#[cfg(feature = "alpen_faucet")]
use alloy::{primitives::Address as AlpenAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use indicatif::ProgressBar;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shrex::{encode, Hex};

#[cfg(feature = "alpen_faucet")]
use crate::{
    net_type::{net_type_or_exit, NetworkType},
    alpen::AlpenWallet,
};
use crate::{seed::Seed, settings::Settings, signet::SignetWallet};

/// Request some bitcoin from the faucet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
pub struct FaucetArgs {
    /// either "signet" or "alpen"
    #[cfg(feature = "alpen_faucet")]
    #[argh(positional)]
    network_type: String,
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
    println!("Fetching challenge from faucet");

    #[cfg(feature = "alpen_faucet")]
    let network_type = net_type_or_exit(&args.network_type);

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
    println!(
        "Received POW challenge with difficulty 2^{} from faucet: {:?}. Solving...",
        challenge.difficulty, challenge.nonce
    );

    let mut solution = 0u64;
    let prehash = {
        let mut hasher = Sha256::new();
        hasher.update(b"alpen faucet 2024");
        hasher.update(challenge.nonce.0);
        hasher
    };
    let pb = ProgressBar::new_spinner();
    let mut counter = 0u64;
    while !pow_valid(
        prehash.clone(),
        challenge.difficulty,
        solution.to_le_bytes(),
    ) {
        solution += 1;
        if counter % 100 == 0 {
            pb.set_message(format!("Trying {solution}"));
        }
        counter += 1;
    }
    pb.finish_with_message(format!(
        "✔ Solved challenge after {solution} attempts. Claiming now."
    ));

    #[cfg(feature = "alpen_faucet")]
    let url = match network_type {
        NetworkType::Signet => {
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .unwrap();
            let address = match args.address {
                None => {
                    let address_info = l1w.reveal_next_address(KeychainKind::External);
                    l1w.persist().unwrap();
                    address_info.address
                }
                Some(address) => {
                    let address = Address::from_str(&address).expect("bad address");
                    address
                        .require_network(settings.network)
                        .expect("wrong bitcoin network")
                }
            };

            println!("Claiming to signet address {}", address);

            format!(
                "{base}claim_l1/{}/{}",
                encode(&solution.to_le_bytes()),
                address
            )
        }
        NetworkType::Alpen => {
            let l2w = AlpenWallet::new(&seed, &settings.alpen_endpoint).unwrap();
            // they said EVMs were advanced 👁️👁️
            let address = match args.address {
                Some(address) => AlpenAddress::from_str(&address).expect("bad address"),
                None => l2w.default_signer_address(),
            };
            println!("Claiming to Alpen address {}", address);
            format!(
                "{base}claim_l2/{}/{}",
                encode(&solution.to_le_bytes()),
                address
            )
        }
    };

    #[cfg(not(feature = "alpen_faucet"))]
    let url = {
        let mut l1w =
            SignetWallet::new(&seed, settings.network, settings.signet_backend.clone()).unwrap();
        let address = match args.address {
            None => {
                let address_info = l1w.reveal_next_address(KeychainKind::External);
                l1w.persist().unwrap();
                address_info.address
            }
            Some(address) => {
                let address = Address::from_str(&address).expect("bad address");
                address
                    .require_network(settings.network)
                    .expect("wrong bitcoin network")
            }
        };

        println!("Claiming to signet address {}", address);

        format!(
            "{base}claim_l1/{}/{}",
            encode(&solution.to_le_bytes()),
            address
        )
    };

    let res = client.get(url).send().await.unwrap();

    let status = res.status();
    let body = res.text().await.expect("invalid response");
    if status == StatusCode::OK {
        println!(
            "Successful queued request for signet bitcoin. It should arrive in your wallet soon.",
        );
    } else {
        println!("Failed: faucet responded with {status}: {body}");
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
