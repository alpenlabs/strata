//! Crate includes reusable utils for services that handle common behavior.
//! Such as initializing the tracing framework and whatever else.

pub mod logging;

#[cfg(feature = "debug-utils")]
pub mod bail_manager;
pub mod ws_client;
