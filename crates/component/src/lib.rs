use component::ClientComponent;
use context::{CsmContext, RunContext};
use sidecar::SideCar;
use strata_db::traits::Database;
use strata_eectl::engine::ExecEngineCtl;

pub mod component;
pub mod context;
pub mod csm_handle;
pub mod sidecar;

pub struct ClientHandle;

/// Trait that should be implemented by a strata client
pub trait Client<
    D: Database + Send + Sync + 'static,
    R: ClientComponent<D>,
    F: ClientComponent<D>,
    C: ClientComponent<D>,
    Ch: ClientComponent<D>,
>
{
    fn from_components(
        reader: R,
        fcm: F,
        csm: C,
        chain: Ch,
        sidecars: Vec<Box<dyn SideCar>>,
    ) -> Self;

    fn run<E: ExecEngineCtl>(&self, runctx: &CsmContext<D, E>) -> ClientHandle;

    // TODO validate
}

/// Trait to facilitate event submission to the csm
pub trait CsmHandle {
    type Event;
    fn submit_event(&self, event: Self::Event) -> anyhow::Result<()>;
    fn submit_event_async(
        &self,
        event: Self::Event,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

impl CsmHandle for () {
    type Event = ();

    fn submit_event(&self, _event: Self::Event) -> anyhow::Result<()> {
        Ok(())
    }

    async fn submit_event_async(&self, _event: Self::Event) -> anyhow::Result<()> {
        Ok(())
    }
}
