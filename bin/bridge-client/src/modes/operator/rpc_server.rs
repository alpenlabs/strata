//! Bootstraps an RPC server for the operator.
use anyhow::Context;
use async_trait::async_trait;
use bitcoin::secp256k1::schnorr::Signature;
use chrono::{DateTime, Utc};
use jsonrpsee::{core::RpcResult, RpcModule};
use tokio::sync::oneshot;
use tracing::{info, warn};

use super::constants::{RPC_PORT, RPC_SERVER};
use express_bridge_rpc_api::{
    ExpressBridgeControlApiServer, ExpressBridgeNetworkApiServer, ExpressBridgeWalletApiServer,
};
use express_bridge_txm::DepositInfo;

pub(crate) async fn start<T>(rpc_impl: &T) -> anyhow::Result<()>
where
    T: ExpressBridgeControlApiServer
        + ExpressBridgeNetworkApiServer
        + ExpressBridgeWalletApiServer
        + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());

    let control_api = ExpressBridgeControlApiServer::into_rpc(rpc_impl.clone());
    let network_api = ExpressBridgeNetworkApiServer::into_rpc(rpc_impl.clone());
    let wallet_api = ExpressBridgeWalletApiServer::into_rpc(rpc_impl.clone());

    rpc_module.merge(control_api).context("merge control api")?;
    rpc_module.merge(network_api).context("merge network api")?;
    rpc_module.merge(wallet_api).context("merge wallet api")?;

    let addr = format!("{RPC_SERVER}:{RPC_PORT}");
    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(&addr)
        .await
        .expect("build bridge rpc server");

    let rpc_handle = rpc_server.start(rpc_module);
    // Using `_` for `_stop_tx` as the variable causes it to be dropped immediately!
    let (_stop_tx, stop_rx): (oneshot::Sender<bool>, oneshot::Receiver<bool>) = oneshot::channel();

    info!("bridge RPC server started at: {addr}");

    let _ = stop_rx.await;
    info!("stopping RPC server");

    if rpc_handle.stop().is_err() {
        warn!("rpc server already stopped");
    }

    Ok(())
}

/// Struct to implement the `express_bridge_rpc_api::ExpressBridgeNetworkApiServer` on. Contains
/// fields corresponding the global context for the RPC.
#[derive(Debug, Clone)]
pub(crate) struct BridgeRpcImpl {
    start_time: DateTime<Utc>,
}

impl Default for BridgeRpcImpl {
    fn default() -> Self {
        Self {
            start_time: Utc::now(),
        }
    }
}

// NOTE: These methods require context regarding the client itself so these have been implemented
// here directly. For all other traits, the impls should go into `express-bridge-rpc-api::services`.
#[async_trait]
impl ExpressBridgeControlApiServer for BridgeRpcImpl {
    async fn get_protocol_version(&self) -> RpcResult<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    async fn get_uptime(&self) -> RpcResult<u64> {
        let current_time = Utc::now().timestamp();
        let start_time = self.start_time.timestamp();

        assert!(current_time >= start_time, "clock cannot move backwards");

        Ok(current_time.abs_diff(start_time))
    }
}

#[async_trait]
impl ExpressBridgeNetworkApiServer for BridgeRpcImpl {
    async fn ping(&self) -> RpcResult<()> {
        unimplemented!("ping")
    }
}

#[async_trait]
impl ExpressBridgeWalletApiServer for BridgeRpcImpl {
    async fn request_signature(&self, _deposit_info: DepositInfo) -> RpcResult<Signature> {
        unimplemented!("request_signature");
    }

    async fn sign_deposit_transaction(
        &self,
        _address: String,
        _deposit_info: DepositInfo,
    ) -> RpcResult<Signature> {
        unimplemented!("sign_deposit_transaction");
    }

    async fn verify_deposit_transaction(
        &self,
        _address: String,
        _deposit_info: DepositInfo,
        _signature: Signature,
    ) -> RpcResult<()> {
        unimplemented!("verify_deposit_transaction");
    }
}
