use crate::context::{BuildContext, ComponentHandle, RunContext};

/// Trait which must be implemented by every component of the system
pub trait ClientComponent {
    // Startup checks and all
    fn validate(&self);
    // Actually run the component
    fn run(&self, runctx: RunContext) -> ComponentHandle;
}

impl ClientComponent for () {
    fn validate(&self) {}

    fn run(&self, runctx: RunContext) -> ComponentHandle {
        ComponentHandle
    }
}

/// Trait which must be implemented by builders of the components
pub trait ComponentBuilder {
    type Output: ClientComponent;
    // Create the component
    fn build(&self, buildctx: &BuildContext) -> Self::Output;
}

impl ComponentBuilder for () {
    type Output = ();
    fn build(&self, _buildctx: &BuildContext) {}
}
