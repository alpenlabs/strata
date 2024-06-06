use crate::auth_client_layer::{AuthClientLayer, AuthClientService};
use crate::el_payload::ElPayload;
use crate::get_runtime;

use alpen_vertex_evmctl::engine::{BlockStatus, ExecEngineCtl, PayloadStatus};
use alpen_vertex_evmctl::errors::{EngineError, EngineResult};
use alpen_vertex_evmctl::messages::{ELDepositData, ExecPayloadData, Op, PayloadEnv};
use alpen_vertex_state::block::L2BlockId;
use jsonrpsee::http_client::{transport::HttpBackend, HttpClient, HttpClientBuilder};
use reth_node_ethereum::EthEngineTypes;
use reth_primitives::{Address, B256};
use reth_rpc::JwtSecret;
use reth_rpc_api::EngineApiClient;
use reth_rpc_types::engine::{
    ExecutionPayloadEnvelopeV2, ExecutionPayloadFieldV2, ExecutionPayloadInputV2, ForkchoiceState, PayloadAttributes, PayloadId, PayloadStatusEnum
};
use reth_rpc_types::Withdrawal;
use tokio::sync::Mutex;

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

pub struct RpcExecEngineCtl {
    client: HttpClient<AuthClientService<HttpBackend>>,
    fork_choice_state: Mutex<ForkchoiceState>,
}

impl RpcExecEngineCtl {
    pub fn new(http_url: &str, secret: &str, fork_choice_state: ForkchoiceState) -> Self {
        Self {
            client: http_client(http_url, secret),
            fork_choice_state: Mutex::new(fork_choice_state),
        }
    }

    fn engine_api_client(&self) -> &impl EngineApiClient<EthEngineTypes> {
        &self.client
    }

    async fn update_block_state(
        &self,
        fcs_partial: ForkchoiceStatePartial,
    ) -> EngineResult<BlockStatus> {
        let fork_choice_state = {
            let default = self.fork_choice_state.lock().await;
            ForkchoiceState {
                head_block_hash: fcs_partial
                    .head_block_hash
                    .unwrap_or(default.head_block_hash),
                safe_block_hash: fcs_partial
                    .safe_block_hash
                    .unwrap_or(default.safe_block_hash),
                finalized_block_hash: fcs_partial
                    .finalized_block_hash
                    .unwrap_or(default.finalized_block_hash),
            }
        };

        let fork_choice_result = self
            .engine_api_client()
            .fork_choice_updated_v2(fork_choice_state, None)
            .await;

        match fork_choice_result {
            Ok(update_status) => {
                match update_status.payload_status.status {
                    PayloadStatusEnum::Valid => {
                        *self.fork_choice_state.lock().await = fork_choice_state;
                        EngineResult::Ok(BlockStatus::Valid)
                    }
                    PayloadStatusEnum::Syncing => EngineResult::Ok(BlockStatus::Syncing),
                    PayloadStatusEnum::Invalid { .. } => EngineResult::Ok(BlockStatus::Invalid),
                    PayloadStatusEnum::Accepted => EngineResult::Err(EngineError::Unimplemented), // should not be called; panic ?
                }
            }
            Err(err) => EngineResult::Err(EngineError::Other(err.to_string())),
        }
    }

    async fn build_block_from_mempool(&self, payload_env: PayloadEnv) -> EngineResult<u64> {
        // TODO: pass other fields from payload_env
        let withdrawals: Vec<Withdrawal> = payload_env
            .el_ops
            .iter()
            .filter_map(|op| match op {
                Op::Deposit(deposit_data) => Some(Withdrawal {
                    address: address_from_vec(deposit_data.dest_addr.clone())?,
                    amount: deposit_data.amt,
                    ..Default::default()
                }),
            })
            .collect();

        let payload_attributes = PayloadAttributes {
            timestamp: payload_env.timestamp,
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
            .engine_api_client()
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
        let payload_result: Result<ExecutionPayloadEnvelopeV2, jsonrpsee::core::ClientError> = self
            .engine_api_client()
            .get_payload_v2(payload_id_bytes)
            .await;

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
                        Op::Deposit(ELDepositData {
                            amt: withdrawal.amount,
                            dest_addr: withdrawal.address.as_slice().to_vec(),
                        })
                    })
                    .collect();

                let el_payload: ElPayload = payload.payload_inner.into();
                ExecPayloadData::new(borsh::to_vec(&el_payload).unwrap(), ops)
            }
        };

        Ok(PayloadStatus::Ready(execution_payload_data))
    }

    async fn submit_new_payload(&self, payload: ExecPayloadData) -> EngineResult<BlockStatus> {
        let el_payload = borsh::from_slice::<ElPayload>(&payload.el_payload).map_err(|_| EngineError::Other("Invalid payload".to_string()))?;

        let withdrawals: Vec<Withdrawal> = payload.ops
            .iter()
            .filter_map(|op| match op {
                Op::Deposit(deposit_data) => Some(Withdrawal {
                    address: address_from_vec(deposit_data.dest_addr.clone())?,
                    amount: deposit_data.amt,
                    ..Default::default()
                }),
            })
            .collect();

        let v2_payload = ExecutionPayloadInputV2 {
            execution_payload: el_payload.into(),
            withdrawals: if withdrawals.is_empty() { None } else { Some(withdrawals) }
        };
        
        let payload_status_result = self.engine_api_client().new_payload_v2(v2_payload).await;

        let payload_status = payload_status_result.map_err(|err| EngineError::Other(err.to_string()))?;

        match payload_status.status {
            PayloadStatusEnum::Valid => EngineResult::Ok(BlockStatus::Valid),
            PayloadStatusEnum::Syncing => EngineResult::Ok(BlockStatus::Syncing),
            PayloadStatusEnum::Invalid { .. } => EngineResult::Ok(BlockStatus::Invalid),
            PayloadStatusEnum::Accepted => EngineResult::Err(EngineError::Unimplemented), // TODO
        }
    }
}

impl ExecEngineCtl for RpcExecEngineCtl {
    fn submit_payload(&self, payload: ExecPayloadData) -> EngineResult<BlockStatus> {
        get_runtime().block_on(self.submit_new_payload(payload))
    }

    fn prepare_payload(&self, env: PayloadEnv) -> EngineResult<u64> {
        get_runtime().block_on(self.build_block_from_mempool(env))
    }

    fn get_payload_status(&self, id: u64) -> EngineResult<PayloadStatus> {
        get_runtime().block_on(self.get_payload_status(id))
    }

    fn update_head_block(&self, id: L2BlockId) -> EngineResult<()> {
        get_runtime().block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                head_block_hash: Some(id.0 .0),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }

    fn update_safe_block(&self, id: L2BlockId) -> EngineResult<()> {
        get_runtime().block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                safe_block_hash: Some(id.0 .0),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }

    fn update_finalized_block(&self, id: L2BlockId) -> EngineResult<()> {
        get_runtime().block_on(async {
            let fork_choice_state = ForkchoiceStatePartial {
                finalized_block_hash: Some(id.0 .0),
                ..Default::default()
            };
            self.update_block_state(fork_choice_state).await.map(|_| ())
        })
    }
}

