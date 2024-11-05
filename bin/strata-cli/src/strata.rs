use std::{
    io,
    ops::{Deref, DerefMut},
};

use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::TxHash,
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, ProviderBuilder, RootProvider,
    },
    transports::http::{Client, Http},
};
use console::{style, Term};

use crate::{seed::Seed, settings::Settings};

pub fn print_strata_explorer_url(
    txid: &TxHash,
    term: &Term,
    settings: &Settings,
) -> Result<(), io::Error> {
    term.write_line(&match settings.blockscout_endpoint {
        Some(ref url) => format!(
            "View transaction at {}",
            style(format!("{url}/tx/{txid}")).blue()
        ),
        None => format!("Transaction ID: {txid}"),
    })
}

// alloy moment 💀
type Provider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;

pub struct StrataWallet(Provider);

impl DerefMut for StrataWallet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for StrataWallet {
    type Target = Provider;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct L2EndpointParseError;

impl StrataWallet {
    pub fn new(seed: &Seed, l2_http_endpoint: &str) -> Result<Self, L2EndpointParseError> {
        let wallet = seed.strata_wallet();

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(l2_http_endpoint.parse().map_err(|_| L2EndpointParseError)?);
        Ok(Self(provider))
    }
}
