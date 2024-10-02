//! Bootstraps an RPC server for the operator.
use std::sync::Arc;

use alpen_express_rpc_types::RpcServerError;
use alpen_express_state::bridge_duties::BridgeDutyStatus;
use anyhow::Context;
use async_trait::async_trait;
use bitcoin::Txid;
use chrono::{DateTime, Utc};
use express_bridge_rpc_api::{
    ExpressBridgeControlApiServer, ExpressBridgeNetworkApiServer, ExpressBridgeTrackerApiServer,
};
use express_storage::ops::bridge_duty::BridgeDutyOps;
use jsonrpsee::{core::RpcResult, RpcModule};
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::constants::{RPC_PORT, RPC_SERVER};

pub(crate) async fn start<T>(rpc_impl: &T) -> anyhow::Result<()>
where
    T: ExpressBridgeControlApiServer
        + ExpressBridgeNetworkApiServer
        + ExpressBridgeTrackerApiServer
        + Clone
        + Sync
        + Send,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());

    let control_api = ExpressBridgeControlApiServer::into_rpc(rpc_impl.clone());
    let network_api = ExpressBridgeNetworkApiServer::into_rpc(rpc_impl.clone());

    rpc_module.merge(control_api).context("merge control api")?;
    rpc_module.merge(network_api).context("merge network api")?;

    let addr = format!("{RPC_SERVER}:{RPC_PORT}");
    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(&addr)
        .await
        .expect("build bridge rpc server");

    let rpc_handle = rpc_server.start(rpc_module);
    // Using `_` for `_stop_tx` as the variable causes it to be dropped immediately!
    // NOTE: The `_stop_tx` should be used by the shutdown manager (see the `express-tasks` crate).
    // At the moment, the impl below just stops the client from stopping.
    let (_stop_tx, stop_rx): (oneshot::Sender<bool>, oneshot::Receiver<bool>) = oneshot::channel();

    info!("bridge RPC server started at: {addr}");

    let _ = stop_rx.await;
    info!("stopping RPC server");

    if rpc_handle.stop().is_err() {
        warn!("rpc server already stopped");
    }

    Ok(())
}

/// Struct to implement the [`express_bridge_rpc_api::ExpressBridgeNetworkApiServer`] on. Contains
/// fields corresponding the global context for the RPC.
#[derive(Clone)]
pub(crate) struct BridgeRpc {
    start_time: DateTime<Utc>,
    duty_ops: Arc<BridgeDutyOps>,
}

impl BridgeRpc {
    pub fn new(duty_ops: Arc<BridgeDutyOps>) -> Self {
        Self {
            start_time: Utc::now(),
            duty_ops,
        }
    }
}

#[async_trait]
impl ExpressBridgeControlApiServer for BridgeRpc {
    async fn get_client_version(&self) -> RpcResult<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    async fn get_uptime(&self) -> RpcResult<u64> {
        let current_time = Utc::now().timestamp();
        let start_time = self.start_time.timestamp();

        // The user might care about their system time being incorrect.
        if current_time <= start_time {
            return Err(jsonrpsee::types::ErrorObjectOwned::owned::<_>(
                -32000,
                "system time may be inaccurate", // `start_time` may have been incorrect too
                Some(current_time.saturating_sub(start_time)),
            ));
        }

        Ok(current_time.abs_diff(start_time))
    }
}

#[async_trait]
impl ExpressBridgeNetworkApiServer for BridgeRpc {
    async fn ping(&self) -> RpcResult<()> {
        unimplemented!("ping")
    }
}

#[async_trait]
impl ExpressBridgeTrackerApiServer for BridgeRpc {
    async fn get_status(&self, txid: Txid) -> RpcResult<Option<BridgeDutyStatus>> {
        Ok(self
            .duty_ops
            .get_status_async(txid)
            .await
            .map_err(RpcServerError::Db)?)
    }
}
