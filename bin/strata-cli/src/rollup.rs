use std::ops::{Deref, DerefMut};

use alloy::{
    network::{Ethereum, EthereumWallet},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, ProviderBuilder, RootProvider,
    },
    transports::http::{Client, Http},
};

use crate::{seed::Seed, settings::SETTINGS};

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

pub struct RollupWallet(Provider);

impl DerefMut for RollupWallet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for RollupWallet {
    type Target = Provider;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct L2EndpointParseError;

impl RollupWallet {
    pub fn new(seed: &Seed) -> Result<Self, L2EndpointParseError> {
        let wallet = seed.rollup_wallet();

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(
                SETTINGS
                    .l2_http_endpoint
                    .parse()
                    .map_err(|_| L2EndpointParseError)?,
            );
        Ok(Self(provider))
    }
}
