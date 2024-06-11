use std::future::Future;

use jsonrpsee::http_client::HttpClient;
use jsonrpsee::http_client::{transport::HttpBackend, HttpClientBuilder};
use reth_node_ethereum::EthEngineTypes;
use reth_primitives::{Address, B256};
use reth_rpc::JwtSecret;
use reth_rpc_api::EngineApiClient;
use reth_rpc_types::engine::{
    ExecutionPayloadEnvelopeV2, ExecutionPayloadFieldV2, ExecutionPayloadInputV2, ForkchoiceState,
    ForkchoiceUpdated, PayloadAttributes, PayloadId, PayloadStatusEnum,
};
use reth_rpc_types::Withdrawal;
use tokio::runtime::Handle;
use tokio::sync::Mutex;

use alpen_vertex_evmctl::engine::{BlockStatus, ExecEngineCtl, PayloadStatus};
use alpen_vertex_evmctl::errors::{EngineError, EngineResult};
use alpen_vertex_evmctl::messages::{ELDepositData, ExecPayloadData, Op, PayloadEnv};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::block::L2BlockId;

use crate::auth_client_layer::{AuthClientLayer, AuthClientService};
use crate::el_payload::ElPayload;

pub trait HttpClientWrap {
    fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> impl Future<Output = Result<ForkchoiceUpdated, jsonrpsee::core::ClientError>>;

    fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> impl Future<Output = Result<ExecutionPayloadEnvelopeV2, jsonrpsee::core::ClientError>>;

    fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> impl Future<Output = Result<reth_rpc_types::engine::PayloadStatus, jsonrpsee::core::ClientError>>;
}

pub struct HttpClientWrapper {
    client: HttpClient<AuthClientService<HttpBackend>>,
}

impl HttpClientWrapper {
    pub fn from_url_secret(http_url: &str, secret_hex: &str) -> Self {
        HttpClientWrapper {
            client: http_client(http_url, secret_hex),
        }
    }

    pub fn inner(&self) -> &HttpClient<AuthClientService<HttpBackend>> {
        &self.client
    }
}

impl HttpClientWrap for HttpClientWrapper {
    fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<PayloadAttributes>,
    ) -> impl Future<Output = Result<ForkchoiceUpdated, jsonrpsee::core::ClientError>> {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::fork_choice_updated_v2::<'_, '_>(&self.client, fork_choice_state, payload_attributes)
    }

    fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> impl Future<Output = Result<ExecutionPayloadEnvelopeV2, jsonrpsee::core::ClientError>>
    {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::get_payload_v2::<'_, '_>(&self.client, payload_id)
    }

    fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> impl Future<Output = Result<reth_rpc_types::engine::PayloadStatus, jsonrpsee::core::ClientError>>
    {
        <HttpClient<AuthClientService<HttpBackend>> as EngineApiClient<EthEngineTypes>>::new_payload_v2::<'_, '_>(&self.client, payload)
    }
}

fn http_client(http_url: &str, secret_hex: &str) -> HttpClient<AuthClientService<HttpBackend>> {
    let secret = JwtSecret::from_hex(secret_hex).unwrap();
    let middleware = tower::ServiceBuilder::new().layer(AuthClientLayer::new(secret));

    HttpClientBuilder::default()
        .set_http_middleware(middleware)
        .build(http_url)
        .expect("Failed to create http client")
}

fn address_from_vec(vec: Vec<u8>) -> Option<Address> {
    let slice: Option<[u8; 20]> = vec.try_into().ok();
    slice.map(Address::from)
}

#[derive(Debug, Default)]
pub struct ForkchoiceStatePartial {
    /// Hash of the head block.
    pub head_block_hash: Option<B256>,
    /// Hash of the safe block.
    pub safe_block_hash: Option<B256>,
    /// Hash of finalized block.
    pub finalized_block_hash: Option<B256>,
}

pub struct RpcExecEngineCtl<T> {
    client: T,
    fork_choice_state: Mutex<ForkchoiceState>,
    handle: Handle,
}

impl<T> RpcExecEngineCtl<T> {
    pub fn new(client: T, fork_choice_state: ForkchoiceState, handle: Handle) -> Self {
        Self {
            client,
            fork_choice_state: Mutex::new(fork_choice_state),
            handle,
        }
    }
}

impl<T: HttpClientWrap> RpcExecEngineCtl<T> {
    async fn update_block_state(
        &self,
        fcs_partial: ForkchoiceStatePartial,
    ) -> EngineResult<BlockStatus> {
        let fork_choice_state = {
            let existing = self.fork_choice_state.lock().await;
            ForkchoiceState {
                head_block_hash: fcs_partial
                    .head_block_hash
                    .unwrap_or(existing.head_block_hash),
                safe_block_hash: fcs_partial
                    .safe_block_hash
                    .unwrap_or(existing.safe_block_hash),
                finalized_block_hash: fcs_partial
                    .finalized_block_hash
                    .unwrap_or(existing.finalized_block_hash),
            }
        };

        let fork_choice_result = self
            .client
            .fork_choice_updated_v2(fork_choice_state, None)
            .await;

        let update_status =
            fork_choice_result.map_err(|err| EngineError::Other(err.to_string()))?;

        match update_status.payload_status.status {
            PayloadStatusEnum::Valid => {
                *self.fork_choice_state.lock().await = fork_choice_state;
                EngineResult::Ok(BlockStatus::Valid)
            }
            PayloadStatusEnum::Syncing => EngineResult::Ok(BlockStatus::Syncing),
            PayloadStatusEnum::Invalid { .. } => EngineResult::Ok(BlockStatus::Invalid),
            PayloadStatusEnum::Accepted => EngineResult::Err(EngineError::Unimplemented),   // should not be possible
        }
    }

    async fn build_block_from_mempool(&self, payload_env: PayloadEnv) -> EngineResult<u64> {
        // TODO: pass other fields from payload_env
        let withdrawals: Vec<Withdrawal> = payload_env
            .el_ops()
            .iter()
            .filter_map(|op| match op {
                Op::Deposit(deposit_data) => Some(Withdrawal {
                    address: address_from_vec(deposit_data.dest_addr().clone())?,
                    amount: deposit_data.amt(),
                    ..Default::default()
                }),
            })
            .collect();

        let payload_attributes = PayloadAttributes {
            timestamp: payload_env.timestamp(),
            prev_randao: B256::ZERO,
            withdrawals: if withdrawals.is_empty() {
                None
            } else {
                Some(withdrawals)
            },
            parent_beacon_block_root: None,
            suggested_fee_recipient: Address::ZERO,
        };

        let forkchoice_result = self
            .client
            .fork_choice_updated_v2(
                self.fork_choice_state.lock().await.clone(),
                Some(payload_attributes),
            )
            .await;

        // TODO: correct error type
        let update_status = forkchoice_result.map_err(|err| EngineError::Other(err.to_string()))?;

        let payload_id = update_status
            .payload_id
            .ok_or(EngineError::Other("payload_id missing".into()))?; // should never happen

        Ok(payload_id.0.into())
    }

    async fn get_payload_status(&self, payload_id: u64) -> EngineResult<PayloadStatus> {
        let payload_id_bytes = PayloadId::new(payload_id.to_be_bytes());
        let payload_result = self.client.get_payload_v2(payload_id_bytes).await;

        let payload = payload_result.map_err(|_| EngineError::UnknownPayloadId(payload_id))?;

        let execution_payload_data = match payload.execution_payload {
            ExecutionPayloadFieldV2::V1(payload) => {
                let el_payload: ElPayload = payload.into();
                ExecPayloadData::new_simple(borsh::to_vec(&el_payload).unwrap())
            }
            ExecutionPayloadFieldV2::V2(payload) => {
                let ops = payload
                    .withdrawals
                    .iter()
                    .map(|withdrawal| {
                        Op::Deposit(ELDepositData::new(
                            withdrawal.amount,
                            withdrawal.address.as_slice().to_vec(),
                        ))
                    })
                    .collect();

                let el_payload: ElPayload = payload.payload_inner.into();
                ExecPayloadData::new(borsh::to_vec(&el_payload).unwrap(), ops)
            }
        };

        Ok(PayloadStatus::Ready(execution_payload_data))
    }

    async fn submit_new_payload(&self, payload: ExecPayloadData) -> EngineResult<BlockStatus> {
        let el_payload = borsh::from_slice::<ElPayload>(payload.el_payload())
            .map_err(|_| EngineError::Other("Invalid payload".to_string()))?;

        let withdrawals: Vec<Withdrawal> = payload
            .ops()
            .iter()
            .filter_map(|op| match op {
                Op::Deposit(deposit_data) => Some(Withdrawal {
                    address: address_from_vec(deposit_data.dest_addr().clone())?,
                    amount: deposit_data.amt(),
                    ..Default::default()
                }),
            })
            .collect();

        let v2_payload = ExecutionPayloadInputV2 {
            execution_payload: el_payload.into(),
            withdrawals: if withdrawals.is_empty() {
                None
            } else {
                Some(withdrawals)
            },
        };

        let payload_status_result = self.client.new_payload_v2(v2_payload).await;

        let payload_status =
            payload_status_result.map_err(|err| EngineError::Other(err.to_string()))?;

        match payload_status.status {
            PayloadStatusEnum::Valid => EngineResult::Ok(BlockStatus::Valid),
            PayloadStatusEnum::Syncing => EngineResult::Ok(BlockStatus::Syncing),
            PayloadStatusEnum::Invalid { .. } => EngineResult::Ok(BlockStatus::Invalid),
            PayloadStatusEnum::Accepted => EngineResult::Err(EngineError::Unimplemented), // TODO
        }
    }
}

impl<T: HttpClientWrap> ExecEngineCtl for RpcExecEngineCtl<T> {
    fn submit_payload(&self, payload: ExecPayloadData) -> EngineResult<BlockStatus> {
        self.handle.block_on(self.submit_new_payload(payload))
    }

    fn prepare_payload(&self, env: PayloadEnv) -> EngineResult<u64> {
        self.handle.block_on(self.build_block_from_mempool(env))
    }

    fn get_payload_status(&self, id: u64) -> EngineResult<PayloadStatus> {
        self.handle.block_on(self.get_payload_status(id))
    }

    fn update_head_block(&self, id: L2BlockId) -> EngineResult<()> {
        self.handle.block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                head_block_hash: Some(Buf32::from(id).into()),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }

    fn update_safe_block(&self, id: L2BlockId) -> EngineResult<()> {
        self.handle.block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                safe_block_hash: Some(Buf32::from(id).into()),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }

    fn update_finalized_block(&self, id: L2BlockId) -> EngineResult<()> {
        self.handle.block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                finalized_block_hash: Some(Buf32::from(id).into()),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use mockall::{mock, predicate::*};
    use rand::{Rng, RngCore};
    use reth_primitives::{Bloom, Bytes, U256};
    use reth_primitives::revm_primitives::FixedBytes;
    use reth_rpc_types::ExecutionPayloadV1;
    
    use alpen_vertex_evmctl::errors::EngineResult;
    use alpen_vertex_evmctl::messages::PayloadEnv;
    use alpen_vertex_primitives::buf::Buf32;

    use super::*;

    mock! {
        HttpClient {}
        impl HttpClientWrap for HttpClient {
            fn fork_choice_updated_v2(
                &self,
                fork_choice_state: ForkchoiceState,
                payload_attributes: Option<PayloadAttributes>,
            ) -> impl Future<Output = Result<ForkchoiceUpdated, jsonrpsee::core::ClientError>>;

            fn get_payload_v2(
                &self,
                payload_id: PayloadId,
            ) -> impl Future<Output = Result<ExecutionPayloadEnvelopeV2, jsonrpsee::core::ClientError>>;

            fn new_payload_v2(
                &self,
                payload: ExecutionPayloadInputV2,
            ) -> impl Future<Output = Result<reth_rpc_types::engine::PayloadStatus, jsonrpsee::core::ClientError>>;
        }
    }

    fn random_el_payload() -> ElPayload {
        let mut rand_data = vec![0u8; 1024];
        rand::thread_rng().fill_bytes(&mut rand_data);
        let mut unstructured = Unstructured::new(&rand_data);
        ElPayload::arbitrary(&mut unstructured).unwrap()
    }

    fn random_execution_payload_v1() -> ExecutionPayloadV1 {
        let mut rng = rand::thread_rng();

        ExecutionPayloadV1 {
            parent_hash: B256::random(),
            fee_recipient: Address::random(),
            state_root: B256::random(),
            receipts_root: B256::random(),
            logs_bloom: Bloom::random(),
            prev_randao: B256::random(),
            block_number: rng.gen(),
            gas_limit: 200000u64,
            gas_used: 10000u64,
            timestamp: rng.gen(),
            extra_data: Bytes::new(),
            base_fee_per_gas: U256::from(50),
            block_hash: B256::random(),
            transactions: vec![],
        }
    }

    #[tokio::test]
    async fn test_update_block_state() {
        let mut mock_client = MockHttpClient::new();

        let fcs_partial = ForkchoiceStatePartial {
            head_block_hash: Some(B256::random()),
            safe_block_hash: None,
            finalized_block_hash: None,
        };

        let fcs = ForkchoiceState {
            head_block_hash: B256::random(),
            safe_block_hash: B256::random(),
            finalized_block_hash: B256::random(),
        };

        mock_client
            .expect_fork_choice_updated_v2()
            .returning(move |_, _| {
                Box::pin(async { Ok(ForkchoiceUpdated::from_status(PayloadStatusEnum::Valid)) })
            });

        let rpc_exec_engine_ctl = RpcExecEngineCtl::new(mock_client, fcs, Handle::current());

        let result = rpc_exec_engine_ctl.update_block_state(fcs_partial).await;

        assert!(matches!(result, EngineResult::Ok(BlockStatus::Valid)));
    }

    #[tokio::test]
    async fn test_build_block_from_mempool() {
        let mut mock_client = MockHttpClient::new();
        let fcs = ForkchoiceState::default();

        mock_client
            .expect_fork_choice_updated_v2()
            .returning(move |_, _| {
                Box::pin(async {
                    Ok(ForkchoiceUpdated::from_status(PayloadStatusEnum::Valid)
                        .with_payload_id(PayloadId::new([1u8; 8])))
                })
            });

        let rpc_exec_engine_ctl = RpcExecEngineCtl::new(mock_client, fcs, Handle::current());

        let timestamp = 0;
        let el_ops = vec![];
        let safe_l1_block = Buf32(FixedBytes::<32>::random());
        let prev_global_state_root = Buf32(FixedBytes::<32>::random());

        let payload_env = PayloadEnv::new(timestamp, prev_global_state_root, safe_l1_block, el_ops);

        let result = rpc_exec_engine_ctl
            .build_block_from_mempool(payload_env)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_payload_status() {
        let mut mock_client = MockHttpClient::new();
        let fcs = ForkchoiceState::default();

        mock_client.expect_get_payload_v2().returning(move |_| {
            Box::pin(async {
                Ok(ExecutionPayloadEnvelopeV2 {
                    execution_payload: ExecutionPayloadFieldV2::V1(random_execution_payload_v1()),
                    block_value: U256::from(100),
                })
            })
        });

        let rpc_exec_engine_ctl = RpcExecEngineCtl::new(mock_client, fcs, Handle::current());

        let result = rpc_exec_engine_ctl.get_payload_status(0).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_submit_new_payload() {
        let mut mock_client = MockHttpClient::new();
        let fcs = ForkchoiceState::default();

        let el_payload = random_el_payload();
        let ops = vec![Op::Deposit(ELDepositData::new(10000, [1u8; 20].into()))];

        let payload_data = ExecPayloadData::new(borsh::to_vec(&el_payload).unwrap(), ops);

        mock_client.expect_new_payload_v2().returning(move |_| {
            Box::pin(async {
                Ok(reth_rpc_types::engine::PayloadStatus {
                    status: PayloadStatusEnum::Valid,
                    latest_valid_hash: None,
                })
            })
        });

        let rpc_exec_engine_ctl = RpcExecEngineCtl::new(mock_client, fcs, Handle::current());

        let result = rpc_exec_engine_ctl.submit_new_payload(payload_data).await;

        assert!(matches!(result, EngineResult::Ok(BlockStatus::Valid)));
    }
}
