use component::ClientComponent;
use context::RunContext;
use sidecar::SideCar;

pub mod component;
pub mod context;
pub mod csm_handle;
pub mod sidecar;

pub struct ClientHandle;

/// Trait that should be implemented by a strata client
pub trait Client<R: ClientComponent, F: ClientComponent, C: ClientComponent, Ch: ClientComponent> {
    fn from_components(
        reader: R,
        fcm: F,
        csm: C,
        chain: Ch,
        sidecars: Vec<Box<dyn SideCar>>,
    ) -> Self;

    fn run(&self, runctx: &RunContext) -> ClientHandle;

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
