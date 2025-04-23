use std::str::FromStr;

use alloy::{
    network::TransactionBuilder,
    primitives::{Address as StrataAddress, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use argh::FromArgs;
use bdk_wallet::bitcoin::{Address, Amount};
use terrors::OneOf;

use crate::{
    constants::SATS_TO_WEI,
    errors::{
        InvalidSignetAddress, InvalidStrataAddress, InvalidStrataEndpoint, SignetTxError,
        SignetWalletError, StrataTxError, UnsupportedNetwork, WrongNetwork,
    },
    handle_or_exit,
    link::{OnchainObject, PrettyPrint},
    net_type::NetworkType,
    seed::Seed,
    settings::Settings,
    signet::{get_fee_rate, log_fee_rate, SignetWallet},
    strata::StrataWallet,
};

/// Send some bitcoin from the internal wallet.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "send")]
pub struct SendArgs {
    /// either "signet" or "strata"
    #[argh(positional)]
    network_type: String,

    /// amount to send in sats
    #[argh(positional)]
    amount: u64,

    /// address to send to
    #[argh(positional)]
    address: String,

    /// override signet fee rate in sat/vbyte. must be >=1
    #[argh(option)]
    fee_rate: Option<u64>,
}

/// Errors that can occur when sending bitcoin from the internal wallet
pub(crate) type SendError = OneOf<(
    InvalidSignetAddress,
    InvalidStrataAddress,
    WrongNetwork,
    InvalidStrataEndpoint,
    SignetTxError,
    StrataTxError,
    SignetWalletError,
    UnsupportedNetwork,
)>;

pub async fn send(args: SendArgs, seed: Seed, settings: Settings) {
    handle_or_exit!(send_inner(args, seed, settings).await);
}

async fn send_inner(args: SendArgs, seed: Seed, settings: Settings) -> Result<(), SendError> {
    let network_type = args.network_type.parse().map_err(OneOf::new)?;

    match network_type {
        NetworkType::Signet => {
            let amount = Amount::from_sat(args.amount);
            let address = Address::from_str(&args.address)
                .map_err(|_| SendError::new(InvalidSignetAddress(args.address.clone())))
                .and_then(|addr| {
                    addr.require_network(settings.network).map_err(|_| {
                        OneOf::new(WrongNetwork {
                            address: args.address.clone(),
                            network: settings.network.to_string(),
                        })
                    })
                })?;
            let mut l1w =
                SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
                    .map_err(|e| {
                        SendError::new(SignetWalletError::new("Failed to load signet wallet", e))
                    })?;
            l1w.sync().await.map_err(|e| {
                SendError::new(SignetWalletError::new("Failed to sync signet wallet", e))
            })?;
            let fee_rate = get_fee_rate(args.fee_rate, settings.signet_backend.as_ref()).await;
            log_fee_rate(&fee_rate);
            let mut psbt = {
                let mut builder = l1w.build_tx();
                builder.add_recipient(address.script_pubkey(), amount);
                builder.fee_rate(fee_rate);
                builder.finish().map_err(|e| {
                    SendError::new(SignetTxError::new("Failed to build signet transaction", e))
                })?
            };
            l1w.sign(&mut psbt, Default::default()).map_err(|e| {
                SendError::new(SignetTxError::new("Failed to sign signet transaction", e))
            })?;
            let tx = psbt.extract_tx().map_err(|e| {
                SendError::new(SignetTxError::new(
                    "Failed to finalize signet transaction",
                    e,
                ))
            })?;
            settings
                .signet_backend
                .broadcast_tx(&tx)
                .await
                .map_err(|e| {
                    SendError::new(SignetTxError::new(
                        "Failed to broadcast signet transaction",
                        e,
                    ))
                })?;
            let txid = tx.compute_txid();
            println!(
                "{}",
                OnchainObject::from(&txid)
                    .with_maybe_explorer(settings.mempool_space_endpoint.as_deref())
                    .pretty(),
            );
        }
        NetworkType::Strata => {
            let l2w =
                StrataWallet::new(&seed, &settings.strata_endpoint).map_err(SendError::new)?;
            let address = StrataAddress::from_str(&args.address)
                .map_err(|_| SendError::new(InvalidStrataAddress(args.address.clone())))?;
            let tx = TransactionRequest::default()
                .with_to(address)
                .with_value(U256::from(args.amount as u128 * SATS_TO_WEI));
            let res = l2w.send_transaction(tx).await.map_err(|e| {
                SendError::new(StrataTxError::new(
                    "Failed to broadcast strata transaction",
                    e,
                ))
            })?;
            println!(
                "{}",
                OnchainObject::from(res.tx_hash())
                    .with_maybe_explorer(settings.blockscout_endpoint.as_deref())
                    .pretty(),
            );
        }
    };

    println!("Sent {} to {}", Amount::from_sat(args.amount), args.address,);
    Ok(())
}
