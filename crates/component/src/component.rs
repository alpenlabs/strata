use strata_db::traits::Database;

use crate::context::{BuildContext, ComponentHandle, RunContext};

/// Trait which must be implemented by every component of the system
pub trait ClientComponent<D: Database> {
    // Startup checks and all
    fn validate(&self);
    // Actually run the component
    fn run(&self, runctx: RunContext<D>) -> ComponentHandle;
}

impl<D: Database> ClientComponent<D> for () {
    fn validate(&self) {}

    fn run(&self, runctx: RunContext<D>) -> ComponentHandle {
        ComponentHandle
    }
}

/// Trait which must be implemented by builders of the components
pub trait ComponentBuilder<D: Database> {
    type Output: ClientComponent<D>;
    // Create the component
    fn build(&self, buildctx: &BuildContext<D>) -> Self::Output;
}

impl<D: Database> ComponentBuilder<D> for () {
    type Output = ();
    fn build(&self, _buildctx: &BuildContext<D>) {}
}
