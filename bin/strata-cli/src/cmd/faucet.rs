use std::{fmt, str::FromStr};

use alloy::{primitives::Address as StrataAddress, providers::WalletProvider};
use argh::FromArgs;
use bdk_wallet::{bitcoin::Address, KeychainKind};
use indicatif::ProgressBar;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shrex::{encode, Hex};
use terrors::OneOf;

use crate::{
    errors::{internal_err, user_err, CliError, InternalError, UserInputError},
    net_type::NetworkType,
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
    strata::StrataWallet,
};

/// Request some bitcoin from the faucet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "faucet")]
pub struct FaucetArgs {
    /// either "signet" or "strata"
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
            NetworkType::Strata => Ok(Chain::L2),
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

pub async fn faucet(args: FaucetArgs, seed: Seed, settings: Settings) -> Result<(), CliError> {
    let network_type = args.network_type.parse().map_err(OneOf::new)?;

    let (address, claim): (String, &str) = match network_type {
        NetworkType::Signet => {
            let addr = resolve_signet_address(&args, &seed, &settings)?;
            (addr.to_string(), "claim_l1")
        }
        NetworkType::Strata => {
            let addr = resolve_strata_address(&args, &seed, &settings)?;
            (addr.to_string(), "claim_l2")
        }
    };

    println!("Fetching challenge from faucet");

    let client = reqwest::Client::new();
    let base = Url::from_str(&settings.faucet_endpoint)
        .map_err(|_| user_err(UserInputError::InvalidFaucetUrl))?;
    let chain = Chain::from_network_type(network_type.clone())
        .map_err(|_| user_err(UserInputError::UnsupportedNetwork))?;
    let endpoint = base
        .join(&format!("/pow_challenge/{chain}"))
        .expect("a valid URL");

    let challenge = client
        .get(endpoint)
        .send()
        .await
        .map_err(internal_err(InternalError::FetchFaucetPowChallenge))?
        .json::<PowChallenge>()
        .await
        .map_err(internal_err(InternalError::ParseFaucetResponse))?;
    println!(
        "Received POW challenge with difficulty 2^{} from faucet: {:?}. Solving...",
        challenge.difficulty, challenge.nonce
    );

    let mut solution = 0u64;
    let prehash = {
        let mut hasher = Sha256::new();
        hasher.update(b"strata faucet 2024");
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
        "{base}{}/{}/{}",
        claim,
        encode(&solution.to_le_bytes()),
        address
    );
    let res = client
        .get(url)
        .send()
        .await
        .map_err(internal_err(InternalError::ClaimFromFaucet))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(internal_err(InternalError::UnexpectedFaucetResponse))?;
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

fn resolve_signet_address(
    args: &FaucetArgs,
    seed: &Seed,
    settings: &Settings,
) -> Result<bdk_wallet::bitcoin::Address, CliError> {
    let mut l1w =
        SignetWallet::new(seed, settings.network, settings.signet_backend.clone()).unwrap();

    match &args.address {
        None => {
            let address_info = l1w.reveal_next_address(KeychainKind::External);
            l1w.persist().unwrap();
            Ok(address_info.address)
        }
        Some(a) => {
            let address = Address::from_str(a)
                .map_err(|_| user_err(UserInputError::InvalidSignetAddress))?
                .require_network(settings.network)
                .map_err(|_| user_err(UserInputError::WrongNetwork))?;

            Ok(address)
        }
    }
}

fn resolve_strata_address(
    args: &FaucetArgs,
    seed: &Seed,
    settings: &Settings,
) -> Result<alloy::primitives::Address, CliError> {
    let l2w = StrataWallet::new(seed, &settings.strata_endpoint).unwrap();

    match &args.address {
        Some(a) => {
            let address = StrataAddress::from_str(a)
                .map_err(|_| user_err(UserInputError::InvalidStrataAddress))?;
            Ok(address)
        }
        None => Ok(l2w.default_signer_address()),
    }
}
