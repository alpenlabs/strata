use std::sync::Arc;

use bitcoin::Address;
use strata_config::btcio::WriterConfig;
use strata_primitives::params::Params;
use strata_status::StatusChannel;

use crate::rpc::traits::WriterRpc;

/// All the items that writer tasks need as context.
#[derive(Debug, Clone)]
pub struct WriterContext<W: WriterRpc> {
    // Params for rollup.
    pub params: Arc<Params>,
    // Btcio specific configuration.
    pub config: Arc<WriterConfig>,
    // Sequencer's address to watch utxos for and spend change amount to.
    pub sequencer_address: Address,
    // Bitcoin client to sign and submit transactions.
    pub client: Arc<W>,
    // Channel for receiving latest states.
    pub status_channel: StatusChannel,
}

impl<W: WriterRpc> WriterContext<W> {
    pub fn new(
        params: Arc<Params>,
        config: Arc<WriterConfig>,
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
