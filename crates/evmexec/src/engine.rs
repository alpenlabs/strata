use crate::auth_client_layer::{AuthClientLayer, AuthClientService};

use alpen_vertex_evmctl::engine::{BlockStatus, ExecEngineCtl, PayloadStatus};
use alpen_vertex_evmctl::errors::{EngineError, EngineResult};
use alpen_vertex_evmctl::messages::{ExecPayloadData, Op, PayloadEnv, ELDepositData};
use alpen_vertex_state::block::L2BlockId;
use borsh::{BorshDeserialize, BorshSerialize};
use jsonrpsee::http_client::{transport::HttpBackend, HttpClient, HttpClientBuilder};
use reth_node_ethereum::EthEngineTypes;
use reth_primitives::{Address, B256};
use reth_rpc_api::EngineApiClient;
use reth_rpc_types::{ExecutionPayloadV1, Withdrawal};
use reth_rpc_types::engine::{ExecutionPayloadEnvelopeV2, ExecutionPayloadFieldV2, ForkchoiceState, PayloadAttributes, PayloadId, PayloadStatusEnum};
use reth_rpc::JwtSecret;


fn http_client(http_url: &str, secret_hex: &str) -> HttpClient<AuthClientService<HttpBackend>> {
    let secret = JwtSecret::from_hex(secret_hex).unwrap();
    let middleware =
    tower::ServiceBuilder::new().layer(AuthClientLayer::new(secret));
    
    HttpClientBuilder::default()
        .set_http_middleware(middleware)
        .build(http_url)
        .expect("Failed to create http client")
}

fn address_from_vec(vec: Vec<u8>) -> Option<Address> {
    let slice: Option<[u8; 20]> = vec.try_into().ok();
    slice.map(Address::from)
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
struct ElPayloadHeader {
    // TODO
    pub block_number: u64
}

impl Into<ElPayloadHeader> for ExecutionPayloadV1 {
    fn into(self) -> ElPayloadHeader {
        // TODO
        ElPayloadHeader {
            block_number: self.block_number,
        }
    }
}

pub struct RpcExecEngineCtl {
    client: HttpClient<AuthClientService<HttpBackend>>,
    fork_choice_state: ForkchoiceState,
}

impl RpcExecEngineCtl {
    pub fn new(http_url: &str, secret: &str, fork_choice_state: ForkchoiceState) -> Self {
        Self {
            client: http_client(http_url, secret),
            fork_choice_state,
        }
    }
    
    fn engine_api_client(&self) -> &impl EngineApiClient<EthEngineTypes> {
        &self.client
    }

    async fn update_block_state(&mut self, fork_choice_state: ForkchoiceState) -> EngineResult<BlockStatus> {
        let fork_choice_result = self.engine_api_client().fork_choice_updated_v2(fork_choice_state, None).await;
        match fork_choice_result {
            Ok(update_status) => {
                match update_status.payload_status.status {
                    PayloadStatusEnum::Valid => {
                        self.fork_choice_state = fork_choice_state;
                        EngineResult::Ok(BlockStatus::Valid)
                    },
                    PayloadStatusEnum::Syncing => EngineResult::Ok(BlockStatus::Syncing),
                    PayloadStatusEnum::Invalid { .. } => EngineResult::Ok(BlockStatus::Invalid),
                    PayloadStatusEnum::Accepted => EngineResult::Err(EngineError::Unimplemented),   // should not be called; panic ?
                }
            }
            Err(err) => EngineResult::Err(EngineError::Other(err.to_string()))
        }
    }

    async fn build_block_from_mempool(&self, payload_env: PayloadEnv) -> EngineResult<u64> {
        // TODO: pass other fields from payload_env
        let withdrawals: Vec<Withdrawal> = payload_env.el_ops.iter().filter_map(|op| {
            match op {
                Op::Deposit(deposit_data) => Some(Withdrawal {
                    address: address_from_vec(deposit_data.dest_addr.clone())?,
                    amount: deposit_data.amt,
                    ..Default::default()
                }),
            }
        }).collect();

        let payload_attributes = PayloadAttributes {
            timestamp: payload_env.timestamp,
            prev_randao: B256::ZERO,
            withdrawals: if withdrawals.is_empty() { None } else { Some(withdrawals) },
            parent_beacon_block_root: None,
            suggested_fee_recipient: Address::ZERO,
        };
        let forkchoice_result = self.engine_api_client().fork_choice_updated_v2(self.fork_choice_state, Some(payload_attributes)).await;
        match forkchoice_result {
            Ok(update_status) => {
                if let Some(_payload_id) = update_status.payload_id {
                    // FIXME: update dependency where payload id is public
                    // Ok(payload_id.0)
                    Ok(0)
                } else {
                    Err(EngineError::Other("".into()))
                }
            }
            _ => Err(EngineError::Other("".into()))
        }
    }

    async fn get_payload_status(&self, payload_id: u64) -> EngineResult<PayloadStatus> {
        let payload_id_bytes = PayloadId::new(payload_id.to_be_bytes());
        let payload_result: Result<ExecutionPayloadEnvelopeV2, jsonrpsee::core::ClientError> = self.engine_api_client().get_payload_v2(payload_id_bytes).await;
        match payload_result {
            Ok(payload) => {
                let execution_payload_data = match payload.execution_payload {
                    ExecutionPayloadFieldV2::V1(payload) => {
                        let el_payload: ElPayloadHeader = payload.into();
                        ExecPayloadData::new_simple(borsh::to_vec(&el_payload).unwrap())
                    },
                    ExecutionPayloadFieldV2::V2(payload) => {
                        let ops = payload.withdrawals.iter().map(
                            |withdrawal| Op::Deposit(ELDepositData { 
                                amt: withdrawal.amount, 
                                dest_addr: withdrawal.address.as_slice().to_vec(),
                            })
                        ).collect();
                        let el_payload: ElPayloadHeader = payload.payload_inner.into();
                        ExecPayloadData::new(borsh::to_vec(&el_payload).unwrap(), ops)
                    },
                };
                Ok(PayloadStatus::Ready(execution_payload_data))
            }
            Err(_) => {
                Err(EngineError::UnknownPayloadId(payload_id))
            }
        }
    }
}

impl ExecEngineCtl for RpcExecEngineCtl {
    fn submit_payload(&self, payload: ExecPayloadData) -> EngineResult<BlockStatus> {
        todo!()
    }

    fn prepare_payload(&self, env: PayloadEnv) -> EngineResult<u64> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(self.build_block_from_mempool(env))
    }

    fn get_payload_status(&self, id: u64) -> EngineResult<PayloadStatus> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(self.get_payload_status(id))
    }

    fn update_head_block(&self, id: L2BlockId) -> EngineResult<()> {
        todo!()
    }

    fn update_safe_block(&self, id: L2BlockId) -> EngineResult<()> {
        todo!()
    }

    fn update_finalized_block(&self, id: L2BlockId) -> EngineResult<()> {
        todo!()
    }
}

