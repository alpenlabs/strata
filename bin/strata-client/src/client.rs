use std::sync::Arc;

use strata_component::{
    component::ClientComponent, context::RunContext, sidecar::SideCar, Client as ClientT,
    ClientHandle,
};
use strata_consensus_logic::genesis;
use strata_db::traits::Database;
use tracing::info;

pub struct Client<LR, F, C, Ch> {
    reader: LR,
    // writer: W,
    fcm: F,
    // rpc: R,
    csm: C,
    chain: Ch,
    sidecars: Vec<Box<dyn SideCar>>,
}

impl<LR, F, C, Ch> Client<LR, F, C, Ch> {
    pub fn run(&self, runctx: &RunContext) -> ClientHandle {
        ClientHandle
    }

    pub fn do_genesis(
        &self,
        runctx: &RunContext,
        database: Arc<impl Database>,
    ) -> anyhow::Result<()> {
        // Check if we have to do genesis.
        if genesis::check_needs_client_init(database.as_ref())? {
            info!("need to init client state!");
            genesis::init_client_state(&runctx.params, database.as_ref())?;
        }
        Ok(())
    }
}

impl<R: ClientComponent, F: ClientComponent, C: ClientComponent, Ch: ClientComponent>
    ClientT<R, F, C, Ch> for Client<R, F, C, Ch>
{
    fn from_components(
        reader: R,
        fcm: F,
        csm: C,
        chain: Ch,
        sidecars: Vec<Box<dyn SideCar>>,
    ) -> Self {
        Self {
            reader,
            fcm,
            csm,
            chain,
            sidecars,
        }
    }

    fn run(&self, runctx: &RunContext) -> ClientHandle {
        ClientHandle
    }
}
