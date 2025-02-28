//! Crate includes reusable utils for services that handle common behavior.
//! Such as initializing the tracing framework and whatever else.

pub mod logging;

#[cfg(feature = "debug-utils")]
pub mod bail_manager;
#[cfg(feature = "debug-utils")]
pub mod worker_pause_manager;
pub mod ws_client;

/// Checks to see if we should bail out.
#[cfg(feature = "debug-utils")]
pub fn check_bail_trigger(s: &str) {
    bail_manager::check_bail_trigger(s);
}

/// Checks to see if we should bail out.
// Stub for when we don't actually want to do anything.
#[cfg(not(feature = "debug-utils"))]
#[inline(always)]
pub fn check_bail_trigger(_s: &str) {
    // nothing
}
