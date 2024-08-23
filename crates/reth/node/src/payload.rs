use std::convert::Infallible;

use alpen_express_rpc_types::WithdrawalIntent;
use reth_chainspec::ChainSpec;
use reth_node_api::{
    payload::{EngineApiMessageVersion, EngineObjectValidationError},
    validate_version_specific_fields, BuiltPayload, PayloadAttributes, PayloadBuilderAttributes,
};
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use reth_primitives::{
    revm_primitives::{BlockEnv, CfgEnvWithHandlerCfg},
    Address, Header, SealedBlock, Withdrawals, B256, U256,
};
use reth_rpc_types::{
    engine::{
        ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4,
        PayloadAttributes as EthPayloadAttributes, PayloadId,
    },
    ExecutionPayloadV1, Withdrawal,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExpressPayloadAttributes {
    /// An inner payload type
    #[serde(flatten)]
    pub inner: EthPayloadAttributes,
    // /// A custom field
    // pub custom: u64,
}

impl PayloadAttributes for ExpressPayloadAttributes {
    fn timestamp(&self) -> u64 {
        self.inner.timestamp()
    }

    fn withdrawals(&self) -> Option<&Vec<Withdrawal>> {
        self.inner.withdrawals()
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.inner.parent_beacon_block_root()
    }

    fn ensure_well_formed_attributes(
        &self,
        chain_spec: &ChainSpec,
        version: EngineApiMessageVersion,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(chain_spec, version, self.into())?;

        Ok(())
    }
}

/// New type around the payload builder attributes type
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpressPayloadBuilderAttributes(pub(crate) EthPayloadBuilderAttributes);

impl PayloadBuilderAttributes for ExpressPayloadBuilderAttributes {
    type RpcPayloadAttributes = ExpressPayloadAttributes;
    type Error = Infallible;

    fn try_new(parent: B256, attributes: ExpressPayloadAttributes) -> Result<Self, Infallible> {
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

    fn cfg_and_block_env(
        &self,
        chain_spec: &ChainSpec,
        parent: &Header,
    ) -> (CfgEnvWithHandlerCfg, BlockEnv) {
        self.0.cfg_and_block_env(chain_spec, parent)
    }
}

#[derive(Debug, Clone)]
pub struct ExpressBuiltPayload {
    pub(crate) inner: EthBuiltPayload,
    pub(crate) withdrawal_intents: Vec<WithdrawalIntent>,
}

impl ExpressBuiltPayload {
    pub(crate) fn new(inner: EthBuiltPayload, withdrawal_intents: Vec<WithdrawalIntent>) -> Self {
        Self {
            inner,
            withdrawal_intents,
        }
    }
}

impl BuiltPayload for ExpressBuiltPayload {
    fn block(&self) -> &SealedBlock {
        self.inner.block()
    }

    fn fees(&self) -> U256 {
        self.inner.fees()
    }
}

impl From<ExpressBuiltPayload> for ExecutionPayloadV1 {
    fn from(value: ExpressBuiltPayload) -> Self {
        value.inner.into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressExecutionPayloadEnvelopeV2 {
    #[serde(flatten)]
    pub inner: ExecutionPayloadEnvelopeV2,
    pub withdrawal_intents: Vec<WithdrawalIntent>,
}

impl From<ExpressBuiltPayload> for ExpressExecutionPayloadEnvelopeV2 {
    fn from(value: ExpressBuiltPayload) -> Self {
        Self {
            inner: value.inner.into(),
            withdrawal_intents: value.withdrawal_intents,
        }
    }
}

impl From<ExpressBuiltPayload> for ExecutionPayloadEnvelopeV3 {
    fn from(value: ExpressBuiltPayload) -> Self {
        value.inner.into()
    }
}

impl From<ExpressBuiltPayload> for ExecutionPayloadEnvelopeV4 {
    fn from(value: ExpressBuiltPayload) -> Self {
        value.inner.into()
    }
}
