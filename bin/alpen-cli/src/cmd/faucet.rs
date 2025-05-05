use std::{fmt, str::FromStr};

use alloy::{primitives::Address as AlpenAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use indicatif::ProgressBar;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shrex::{encode, Hex};

use crate::{
    alpen::AlpenWallet,
    errors::{DisplayableError, DisplayedError},
    net_type::NetworkType,
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
};

/// Request some bitcoin from the faucet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
pub struct FaucetArgs {
    /// either "signet" or "alpen"
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

/// Which chain the faucet is reasoning about.
enum Chain {
    L1,
    L2,
}

impl Chain {
    fn from_network_type(network_type: NetworkType) -> Result<Self, String> {
        match network_type {
            NetworkType::Signet => Ok(Chain::L1),
            NetworkType::Alpen => Ok(Chain::L2),
        }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chain_str = match self {
            Chain::L1 => "l1",
            Chain::L2 => "l2",
        };
        write!(f, "{}", chain_str)
    }
}

pub async fn faucet(
    args: FaucetArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), DisplayedError> {
    let network_type = args
        .network_type
        .parse()
        .user_error("invalid network type")?;

    let (address, claim) = match network_type {
        NetworkType::Signet => {
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .internal_error("Failed to load signet wallet")?;

            let addr = match &args.address {
                None => {
                    let address_info = l1w.reveal_next_address(KeychainKind::External);
                    l1w.persist()
                        .internal_error("Failed to persist signet wallet")?;
                    address_info.address
                }
                Some(a) => {
                    let unchecked = Address::from_str(a).user_error(format!(
                        "Invalid signet address '{}'. Must be a valid Bitcoin address",
                        a
                    ))?;

                    unchecked
                        .require_network(settings.network)
                        .user_error(format!(
                            "Provided address {} is not valid for network '{}'",
                            a, settings.network
                        ))?
                }
            };
            (addr.to_string(), "claim_l1")
        }
        NetworkType::Alpen => {
            let l2w = AlpenWallet::new(&seed, &settings.alpen_endpoint)
                .user_error("Invalid Alpen endpoint URL. Check the configuration.")?;
            let addr = match &args.address {
                Some(a) => AlpenAddress::from_str(a).user_error(format!(
                    "Invalid Alpen address {}. Must be an EVM-compatible address",
                    a
                ))?,
                None => l2w.default_signer_address(),
            };
            (addr.to_string(), "claim_l2")
        }
    };

    println!("Fetching challenge from faucet");

    let client = reqwest::Client::new();
    let base = Url::from_str(&settings.faucet_endpoint).user_error(format!(
        "Invalid faucet endopoint {}. Check the config file",
        settings.faucet_endpoint.clone()
    ))?;
    let chain = Chain::from_network_type(network_type.clone()).user_error(format!(
        "Unsupported network {}. Must be `signet` or `alpen`",
        network_type
    ))?;
    let endpoint = base
        .join(&format!("/pow_challenge/{chain}"))
        .expect("a valid URL");

    let res = client
        .get(endpoint)
        .send()
        .await
        .internal_error("Failed to fetch PoW challenge")?;

    if !res.status().is_success() {
        let faucet_error = res.text().await.unwrap_or("Unknown error".to_string());
        return Err(DisplayedError::InternalError(
            "Faucet error".to_string(),
            Box::new(faucet_error),
        ));
    }

    let challenge = res
        .json::<PowChallenge>()
        .await
        .internal_error("Failed to parse faucet response")?;
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

    println!("Claiming to {} address {}", network_type, address);

    let url = format!(
        "{base}/{}/{}/{}",
        claim,
        encode(&solution.to_le_bytes()),
        address
    );
    let res = client
        .get(url)
        .send()
        .await
        .internal_error("Failed to claim from faucet")?;

    let status = res.status();
    let body = res
        .text()
        .await
        .internal_error("Failed to parse faucet response")?;
    if status == StatusCode::OK {
        println!("Faucet claim successfully queued. The funds should appear in your wallet soon.",);
    } else {
        println!("Failed: faucet responded with {status}: {body}");
    }

    Ok(())
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
