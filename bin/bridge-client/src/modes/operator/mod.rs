//! Defines the main loop for the bridge-client in operator mode.

#[allow(unused)] // FIXME: remove once these imports are used
use crate::rpc_server::{self, BridgeRpc};

/// Bootstraps the bridge client in Operator mode by hooking up all the required auxiliary services
/// including database, rpc server, etc. Threadpool and logging need to be initialized at the call
/// site (main function) itself.
pub(crate) async fn bootstrap() -> anyhow::Result<()> {
    // Initialize a rocksdb instance with the required column families.
    // Get the `duty_ops` using `into_ops` on the `BridgeDutyDB`
    // let duty_ops = bridge_duty_db.into_ops();
    // let rpc_impl = BridgeRpc::new(duty_ops);

    // rpc_server::start(&rpc_impl).await?;

    Ok(())
}
