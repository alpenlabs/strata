use std::{sync::Arc, time::Duration};

use strata_component::{
    component::ClientComponent,
    context::{CsmContext, RunContext},
    csm_handle::ClientUpdateNotif,
    sidecar::SideCar,
    Client as ClientT, ClientHandle,
};
use strata_consensus_logic::{
    csm::worker::{client_worker_task, WorkerState},
    genesis,
};
use strata_db::traits::Database;
use strata_eectl::{engine::ExecEngineCtl, stub::StubController};
use tokio::sync::broadcast;
use tracing::info;

pub struct Client<LR, F, C, Ch> {
    reader: LR,
    fcm: F,
    csm: C,
    chain: Ch,
    sidecars: Vec<Box<dyn SideCar>>,
}

impl<LR, F, C, Ch> Client<LR, F, C, Ch> {
    pub fn do_genesis<D: Database + Send + Sync + 'static, E: ExecEngineCtl>(
        &self,
        csm: &CsmContext<D, E>,
        database: Arc<impl Database>,
    ) -> anyhow::Result<()> {
        // Check if we have to do genesis.
        if genesis::check_needs_client_init(database.as_ref())? {
            info!("need to init client state!");
            genesis::init_client_state(&csm.params, database.as_ref())?;
        }
        Ok(())
    }

    pub fn run_csm<D: Database + Send + Sync + 'static, E: ExecEngineCtl>(
        &self,
        csmctx: CsmContext<D, E>,
    ) -> anyhow::Result<RunContext<D>> {
        let CsmContext {
            config,
            params,
            db_manager,
            task_manager,
            status_channel,
            csm_handle,
            csm_rx,
            database,
            ..
        } = csmctx;

        // Prepare the client worker state and start the thread for that.
        let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ClientUpdateNotif>>(64);
        let client_worker_state = WorkerState::open(
            params.clone().into(),
            database.clone(),
            db_manager.l2().clone(),
            cupdate_tx.clone(),
            db_manager.checkpoint().clone(),
        )?;

        // TODO: replace with actual engine
        let engine = StubController::new(Duration::from_secs(3));

        let csm_engine = Arc::new(engine);
        let st_ch = status_channel.clone();

        task_manager
            .executor()
            .spawn_critical("client_worker_task", move |shutdown| {
                client_worker_task(shutdown, client_worker_state, csm_engine, csm_rx, st_ch)
                    .map_err(Into::into)
            });

        Ok(RunContext {
            config,
            params,
            db_manager,
            task_manager,
            status_channel,
            csm_handle,
            cupdate_rx,
        })
    }
}

impl<
        D: Database + Send + Sync + 'static,
        R: ClientComponent<D>,
        F: ClientComponent<D>,
        C: ClientComponent<D>,
        Ch: ClientComponent<D>,
    > ClientT<D, R, F, C, Ch> for Client<R, F, C, Ch>
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

    fn run<E: ExecEngineCtl>(&self, runctx: &CsmContext<D, E>) -> ClientHandle {
        ClientHandle
    }
}
