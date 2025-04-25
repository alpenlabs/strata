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

use crate::{
    errors::{user_error, DisplayedError},
    seed::Seed,
};

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

impl StrataWallet {
    pub fn new(seed: &Seed, l2_http_endpoint: &str) -> Result<Self, DisplayedError> {
        let wallet = seed.get_strata_wallet();

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(l2_http_endpoint.parse().map_err(|_| {
                user_error(format!("Invalid strata endpoint: '{}'.", l2_http_endpoint))
            })?);
        Ok(Self(provider))
    }
}
