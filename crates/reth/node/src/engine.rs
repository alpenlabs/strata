use alloy_rpc_types::engine::{
    ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4, ExecutionPayloadV1,
};
use reth_node_api::{payload::PayloadTypes, EngineTypes};
use serde::{Deserialize, Serialize};

use super::payload::{
    StrataBuiltPayload, StrataExecutionPayloadEnvelopeV2, StrataPayloadAttributes,
    StrataPayloadBuilderAttributes,
};

/// Custom engine types for strata to use custom payload attributes and payload
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StrataEngineTypes;

impl PayloadTypes for StrataEngineTypes {
    type BuiltPayload = StrataBuiltPayload;
    type PayloadAttributes = StrataPayloadAttributes;
    type PayloadBuilderAttributes = StrataPayloadBuilderAttributes;
}

impl EngineTypes for StrataEngineTypes {
    type ExecutionPayloadV1 = ExecutionPayloadV1;
    type ExecutionPayloadV2 = StrataExecutionPayloadEnvelopeV2;
    type ExecutionPayloadV3 = ExecutionPayloadEnvelopeV3;
    type ExecutionPayloadV4 = ExecutionPayloadEnvelopeV4;
}
