use std::sync::Arc;

use bitcoin::Address;
use strata_config::btcio::BtcIOConfig;
use strata_primitives::params::Params;
use strata_status::StatusChannel;

use crate::rpc::traits::{Reader, Signer, Wallet};

#[derive(Debug, Clone)]
pub struct WriterContext<T: Reader + Wallet + Signer> {
    pub params: Arc<Params>,
    pub config: Arc<BtcIOConfig>,
    pub sequencer_address: Address,
    pub client: Arc<T>,
    pub status_channel: StatusChannel,
}

impl<T: Reader + Wallet + Signer> WriterContext<T> {
    pub fn new(
        params: Arc<Params>,
        config: Arc<BtcIOConfig>,
        sequencer_address: Address,
        client: Arc<T>,
        status_channel: StatusChannel,
    ) -> Self {
        Self {
            params,
            config,
            sequencer_address,
            client,
            status_channel,
        }
    }
}
