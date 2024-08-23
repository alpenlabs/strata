use reth_chainspec::ChainSpec;
use reth_node_api::{
    payload::{
        EngineApiMessageVersion, EngineObjectValidationError, PayloadOrAttributes, PayloadTypes,
    },
    validate_version_specific_fields, EngineTypes,
};
use reth_rpc_types::{
    engine::{ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4},
    ExecutionPayloadV1,
};
use serde::{Deserialize, Serialize};

use super::payload::{
    ExpressBuiltPayload, ExpressExecutionPayloadEnvelopeV2, ExpressPayloadAttributes,
    ExpressPayloadBuilderAttributes,
};

/// Custom engine types - uses a custom payload attributes RPC type, but uses the default
/// payload builder attributes type.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CustomEngineTypes;

impl PayloadTypes for CustomEngineTypes {
    type BuiltPayload = ExpressBuiltPayload;
    type PayloadAttributes = ExpressPayloadAttributes;
    type PayloadBuilderAttributes = ExpressPayloadBuilderAttributes;
}

impl EngineTypes for CustomEngineTypes {
    type ExecutionPayloadV1 = ExecutionPayloadV1;
    type ExecutionPayloadV2 = ExpressExecutionPayloadEnvelopeV2;
    type ExecutionPayloadV3 = ExecutionPayloadEnvelopeV3;
    type ExecutionPayloadV4 = ExecutionPayloadEnvelopeV4;

    fn validate_version_specific_fields(
        chain_spec: &ChainSpec,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, ExpressPayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(chain_spec, version, payload_or_attrs)
    }
}
