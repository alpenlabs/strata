use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

pub mod preloaded_storage;
pub mod engine;
pub mod auth_client_layer;
mod el_payload;


static SHARED_TOKIO_RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

pub fn get_runtime() -> Arc<Runtime> {
    SHARED_TOKIO_RUNTIME.get_or_init(|| {
        Arc::new(Runtime::new().unwrap())
    }).clone()
}
