//! Defines the main loop for the bridge-client in operator mode.

use crate::rpc_server::{self, BridgeRpc};

/// Bootstraps the bridge client in Operator mode by hooking up all the required auxiliary services
/// including database, rpc server, etc. Threadpool and logging need to be initialized at the call
/// site (main function) itself.
pub(crate) async fn bootstrap() -> anyhow::Result<()> {
    let rpc_impl = BridgeRpc::default();

    rpc_server::start(&rpc_impl).await?;

    Ok(())
}
