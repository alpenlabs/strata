use std::sync::Arc;

use bitcoin::Address;
use strata_config::btcio::BtcIOConfig;
use strata_primitives::params::Params;
use strata_status::StatusChannel;

use crate::rpc::BitcoinClient;

#[derive(Debug, Clone)]
pub struct WriterContext {
    pub params: Arc<Params>,
    pub config: Arc<BtcIOConfig>,
    pub sequencer_address: Address,
    pub client: Arc<BitcoinClient>,
    pub status_channel: StatusChannel,
}

impl WriterContext {
    pub fn new(
        params: Arc<Params>,
        config: Arc<BtcIOConfig>,
        sequencer_address: Address,
        client: Arc<BitcoinClient>,
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
