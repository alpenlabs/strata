use std::sync::Arc;

use alloy_rpc_types::engine::{
    payload::ExecutionData, ExecutionPayload, ExecutionPayloadEnvelopeV3,
    ExecutionPayloadEnvelopeV4, ExecutionPayloadV1,
};
use reth_chainspec::ChainSpec;
use reth_ethereum_payload_builder::EthereumExecutionPayloadValidator;
use reth_node_api::{
    payload::PayloadTypes, validate_version_specific_fields, AddOnsContext, BuiltPayload,
    EngineApiMessageVersion, EngineObjectValidationError, EngineTypes, EngineValidator,
    NewPayloadError, NodeTypes, PayloadOrAttributes, PayloadValidator,
};
use reth_node_builder::{rpc::EngineValidatorBuilder, FullNodeComponents};
use reth_primitives::{Block, NodePrimitives, RecoveredBlock};
use serde::{Deserialize, Serialize};

use super::payload::{StrataBuiltPayload, StrataPayloadBuilderAttributes};
use crate::{node::StrataPrimitives, StrataExecutionPayloadEnvelopeV2, StrataPayloadAttributes};

/// Custom engine types for strata to use custom payload attributes and payload
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StrataEngineTypes<T: PayloadTypes = StrataPayloadTypes> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: PayloadTypes> PayloadTypes for StrataEngineTypes<T> {
    type ExecutionData = ExecutionData;
    type BuiltPayload = T::BuiltPayload;
    type PayloadAttributes = T::PayloadAttributes;
    type PayloadBuilderAttributes = T::PayloadBuilderAttributes;

    fn block_to_payload(
        block: reth_primitives_traits::SealedBlock<
            <<Self::BuiltPayload as BuiltPayload>::Primitives as NodePrimitives>::Block,
        >,
    ) -> Self::ExecutionData {
        let (payload, sidecar) =
            ExecutionPayload::from_block_unchecked(block.hash(), &block.into_block());
        ExecutionData { payload, sidecar }
    }
}

impl<T: PayloadTypes> EngineTypes for StrataEngineTypes<T>
where
    T::BuiltPayload: BuiltPayload<Primitives: NodePrimitives<Block = reth_primitives::Block>>
        + TryInto<ExecutionPayloadV1>
        + TryInto<StrataExecutionPayloadEnvelopeV2>
        + TryInto<ExecutionPayloadEnvelopeV3>
        + TryInto<ExecutionPayloadEnvelopeV4>,
{
    type ExecutionPayloadEnvelopeV1 = ExecutionPayloadV1;
    type ExecutionPayloadEnvelopeV2 = StrataExecutionPayloadEnvelopeV2;
    type ExecutionPayloadEnvelopeV3 = ExecutionPayloadEnvelopeV3;
    type ExecutionPayloadEnvelopeV4 = ExecutionPayloadEnvelopeV4;
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StrataPayloadTypes;

impl PayloadTypes for StrataPayloadTypes {
    type ExecutionData = ExecutionData;
    type BuiltPayload = StrataBuiltPayload;
    type PayloadAttributes = StrataPayloadAttributes;
    type PayloadBuilderAttributes = StrataPayloadBuilderAttributes;

    fn block_to_payload(
        block: reth_primitives_traits::SealedBlock<
            <<Self::BuiltPayload as BuiltPayload>::Primitives as NodePrimitives>::Block,
        >,
    ) -> Self::ExecutionData {
        let (payload, sidecar) =
            ExecutionPayload::from_block_unchecked(block.hash(), &block.into_block());
        ExecutionData { payload, sidecar }
    }
}

/// Strata engine validator
#[derive(Debug, Clone)]
pub struct StrataEngineValidator {
    inner: EthereumExecutionPayloadValidator<ChainSpec>,
}

impl StrataEngineValidator {
    /// Instantiates a new validator.
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthereumExecutionPayloadValidator::new(chain_spec),
        }
    }

    /// Returns the chain spec used by the validator.
    #[inline]
    fn chain_spec(&self) -> &ChainSpec {
        self.inner.chain_spec()
    }
}

impl PayloadValidator for StrataEngineValidator {
    type Block = Block;
    type ExecutionData = ExecutionData;

    fn ensure_well_formed_payload(
        &self,
        payload: ExecutionData,
    ) -> Result<RecoveredBlock<Self::Block>, NewPayloadError> {
        // let sealed_block = self.inner.ensure_well_formed_payload(payload)?;
        let sealed_block = self.inner.ensure_well_formed_payload(payload)?;
        sealed_block
            .try_recover()
            .map_err(|e| NewPayloadError::Other(e.into()))
    }
}

impl<T> EngineValidator<T> for StrataEngineValidator
where
    T: EngineTypes<PayloadAttributes = StrataPayloadAttributes, ExecutionData = ExecutionData>,
{
    fn validate_version_specific_fields(
        &self,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, Self::ExecutionData, T::PayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(self.chain_spec(), version, payload_or_attrs)
    }

    fn ensure_well_formed_attributes(
        &self,
        _version: EngineApiMessageVersion,
        _attributes: &T::PayloadAttributes,
    ) -> Result<(), EngineObjectValidationError> {
        // validate_version_specific_fields(self.chain_spec(), version, attributes.into())?;
        // Ok(())
        todo!("validate version specific fields");
    }
}

/// Builder for [`StrataEngineValidator`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct StrataEngineValidatorBuilder;

impl<Node, Types> EngineValidatorBuilder<Node> for StrataEngineValidatorBuilder
where
    Types: NodeTypes<
        ChainSpec = ChainSpec,
        Payload = StrataEngineTypes,
        Primitives = StrataPrimitives,
    >,
    Node: FullNodeComponents<Types = Types>,
{
    type Validator = StrataEngineValidator;

    async fn build(self, ctx: &AddOnsContext<'_, Node>) -> eyre::Result<Self::Validator> {
        Ok(StrataEngineValidator::new(ctx.config.chain.clone()))
    }
}
