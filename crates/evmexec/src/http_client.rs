use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use jsonrpsee::http_client::{transport::HttpBackend, HttpClientBuilder};
use reth_node_ethereum::EthEngineTypes;
use reth_primitives::{Block, BlockHash};
use reth_rpc_api::{EngineApiClient, EthApiClient};
use reth_rpc_layer::{AuthClientLayer, AuthClientService};
use reth_rpc_types::engine::{
    ExecutionPayloadBodiesV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadInputV2, ForkchoiceState,
    ForkchoiceUpdated, JwtSecret, PayloadAttributes, PayloadId,
};

#[cfg(test)]
use mockall::automock;

fn http_client(http_url: &str, secret: JwtSecret) -> HttpClient<AuthClientService<HttpBackend>> {
    let middleware = tower::ServiceBuilder::new().layer(AuthClientLayer::new(secret));

    HttpClientBuilder::default()
        .set_http_middleware(middleware)
        .build(http_url)
        .expect("Failed to create http client")
}

type RpcResult<T> = Result<T, jsonrpsee::core::ClientError>;

#[allow(async_fn_in_trait)]
#[cfg_attr(test, automock)]
pub trait EngineRpc {
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated>;

    async fn get_payload_v2(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadEnvelopeV2>;

    async fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> RpcResult<reth_rpc_types::engine::PayloadStatus>;

    async fn get_payload_bodies_by_hash_v1(
        &self,
        block_hashes: Vec<BlockHash>,
    ) -> RpcResult<ExecutionPayloadBodiesV1>;

    async fn block_by_hash(&self, block_hash: BlockHash) -> RpcResult<Option<Block>>;
}

#[derive(Debug, Clone)]
pub struct EngineRpcClient {
    client: Arc<HttpClient<AuthClientService<HttpBackend>>>,
}

impl EngineRpcClient {
    pub fn from_url_secret(http_url: &str, secret: JwtSecret) -> Self {
        EngineRpcClient {
            client: Arc::new(http_client(http_url, secret)),
        }
    }

    pub fn inner(&self) -> &HttpClient<AuthClientService<HttpBackend>> {
        &self.client
    }
}

impl EngineRpc for EngineRpcClient {
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v2(&self.client, fork_choice_state, payload_attributes).await
    }

    async fn get_payload_v2(&self, payload_id: PayloadId) -> RpcResult<ExecutionPayloadEnvelopeV2> {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::get_payload_v2(&self.client, payload_id).await
    }

    async fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> RpcResult<reth_rpc_types::engine::PayloadStatus> {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::new_payload_v2(&self.client, payload).await
    }

    async fn get_payload_bodies_by_hash_v1(
        &self,
        block_hashes: Vec<BlockHash>,
    ) -> RpcResult<ExecutionPayloadBodiesV1> {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::get_payload_bodies_by_hash_v1(&self.client, block_hashes).await
    }

    async fn block_by_hash(&self, block_hash: BlockHash) -> RpcResult<Option<Block>> {
        let block = self.client.block_by_hash(block_hash, true).await?;

        Ok(block.map(|rich_block| rich_block.inner.try_into().unwrap()))
    }
}
