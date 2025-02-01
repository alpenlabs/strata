use std::convert::Infallible;

use alloy_eips::{eip4895::Withdrawals, eip7685::Requests};
use alloy_rpc_types::{
    engine::{
        ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4, ExecutionPayloadV1,
        ExecutionPayloadV2, PayloadAttributes as EthPayloadAttributes, PayloadId,
    },
    Withdrawal,
};
use reth::rpc::compat::engine::payload::block_to_payload_v2;
use reth_chain_state::ExecutedBlock;
use reth_node_api::{BuiltPayload, PayloadAttributes, PayloadBuilderAttributes};
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth_primitives::{EthPrimitives, SealedBlock};
use revm_primitives::alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use strata_reth_primitives::WithdrawalIntent;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StrataPayloadAttributes {
    /// An inner payload type
    #[serde(flatten)]
    pub inner: EthPayloadAttributes,
    // additional custom fields to be added
}

impl StrataPayloadAttributes {
    pub fn new_from_eth(payload_attributes: EthPayloadAttributes) -> Self {
        Self {
            inner: payload_attributes,
            // more fields here
        }
    }
}

impl PayloadAttributes for StrataPayloadAttributes {
    fn timestamp(&self) -> u64 {
        self.inner.timestamp()
    }

    fn withdrawals(&self) -> Option<&Vec<Withdrawal>> {
        self.inner.withdrawals()
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.inner.parent_beacon_block_root()
    }
}

/// New type around the payload builder attributes type
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrataPayloadBuilderAttributes(pub(crate) EthPayloadBuilderAttributes);

impl PayloadBuilderAttributes for StrataPayloadBuilderAttributes {
    type RpcPayloadAttributes = StrataPayloadAttributes;
    type Error = Infallible;

    fn try_new(
        parent: B256,
        attributes: StrataPayloadAttributes,
        _version: u8,
    ) -> Result<Self, Infallible> {
        Ok(Self(EthPayloadBuilderAttributes::new(
            parent,
            attributes.inner,
        )))
    }

    fn payload_id(&self) -> PayloadId {
        self.0.id
    }

    fn parent(&self) -> B256 {
        self.0.parent
    }

    fn timestamp(&self) -> u64 {
        self.0.timestamp
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.0.parent_beacon_block_root
    }

    fn suggested_fee_recipient(&self) -> Address {
        self.0.suggested_fee_recipient
    }

    fn prev_randao(&self) -> B256 {
        self.0.prev_randao
    }

    fn withdrawals(&self) -> &Withdrawals {
        &self.0.withdrawals
    }
}

#[derive(Debug, Clone)]
pub struct StrataBuiltPayload {
    pub(crate) inner: EthBuiltPayload,
    pub(crate) withdrawal_intents: Vec<WithdrawalIntent>,
}

impl StrataBuiltPayload {
    pub(crate) fn new(inner: EthBuiltPayload, withdrawal_intents: Vec<WithdrawalIntent>) -> Self {
        Self {
            inner,
            withdrawal_intents,
        }
    }
}

impl BuiltPayload for StrataBuiltPayload {
    type Primitives = EthPrimitives;

    fn block(&self) -> &SealedBlock {
        self.inner.block()
    }

    fn fees(&self) -> U256 {
        self.inner.fees()
    }

    fn executed_block(&self) -> Option<ExecutedBlock> {
        self.inner.executed_block()
    }

    fn requests(&self) -> Option<Requests> {
        self.inner.requests()
    }
}

impl From<StrataBuiltPayload> for ExecutionPayloadV1 {
    fn from(value: StrataBuiltPayload) -> Self {
        value.inner.into()
    }
}

/// Custom Execution payload v2

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadEnvelopeV2 {
    /// Execution payload, which could be either V1 or V2
    ///
    /// V1 (_NO_ withdrawals) MUST be returned if the payload timestamp is lower than the Shanghai
    /// timestamp
    ///
    /// V2 (_WITH_ withdrawals) MUST be returned if the payload timestamp is greater or equal to
    /// the Shanghai timestamp
    pub execution_payload: ExecutionPayloadFieldV2,
    /// The expected value to be received by the feeRecipient in wei
    pub block_value: U256,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExecutionPayloadFieldV2 {
    /// V2 payload
    V2(ExecutionPayloadV2),
    /// V1 payload
    V1(ExecutionPayloadV1),
}

impl ExecutionPayloadFieldV2 {
    /// Returns the inner [ExecutionPayloadV1]
    pub fn into_v1_payload(self) -> ExecutionPayloadV1 {
        match self {
            Self::V2(payload) => payload.payload_inner,
            Self::V1(payload) => payload,
        }
    }
}

impl From<EthBuiltPayload> for ExecutionPayloadEnvelopeV2 {
    fn from(value: EthBuiltPayload) -> Self {
        let block = value.block().clone();
        let fees = value.fees();

        Self {
            block_value: fees,
            execution_payload: ExecutionPayloadFieldV2::V2(block_to_payload_v2(block)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrataExecutionPayloadEnvelopeV2 {
    #[serde(flatten)]
    pub inner: ExecutionPayloadEnvelopeV2,
    pub withdrawal_intents: Vec<WithdrawalIntent>,
}

impl StrataExecutionPayloadEnvelopeV2 {
    pub fn inner(&self) -> &ExecutionPayloadEnvelopeV2 {
        &self.inner
    }
}

impl From<StrataBuiltPayload> for StrataExecutionPayloadEnvelopeV2 {
    fn from(value: StrataBuiltPayload) -> Self {
        Self {
            inner: value.inner.into(),
            withdrawal_intents: value.withdrawal_intents,
        }
    }
}

impl From<StrataBuiltPayload> for ExecutionPayloadEnvelopeV2 {
    fn from(value: StrataBuiltPayload) -> Self {
        value.inner.into()
    }
}

impl From<StrataBuiltPayload> for ExecutionPayloadEnvelopeV3 {
    fn from(value: StrataBuiltPayload) -> Self {
        value.inner.into()
    }
}

impl From<StrataBuiltPayload> for ExecutionPayloadEnvelopeV4 {
    fn from(value: StrataBuiltPayload) -> Self {
        value.inner.into()
    }
}
