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
};

use crate::seed::Seed;

// alloy moment 💀
type Provider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Ethereum>,
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
        let wallet = seed.get_strata_wallet();

        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .on_http(l2_http_endpoint.parse().map_err(|_| L2EndpointParseError)?);
        Ok(Self(provider))
    }
}
