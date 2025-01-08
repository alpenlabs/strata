use std::sync::Arc;

use bitcoin::Address;
use strata_config::btcio::BtcioConfig;
use strata_primitives::params::Params;
use strata_status::StatusChannel;

use crate::rpc::traits::WriterRpc;

#[derive(Debug, Clone)]
pub struct WriterContext<W: WriterRpc> {
    pub params: Arc<Params>,
    pub config: Arc<BtcioConfig>,
    pub sequencer_address: Address,
    pub client: Arc<W>,
    pub status_channel: StatusChannel,
}

impl<W: WriterRpc> WriterContext<W> {
    pub fn new(
        params: Arc<Params>,
        config: Arc<BtcioConfig>,
        sequencer_address: Address,
        client: Arc<W>,
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
