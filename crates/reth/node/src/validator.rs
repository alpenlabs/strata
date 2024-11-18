use std::sync::Arc;

use reth::builder::{components::EngineValidatorBuilder, BuilderContext};
use reth_chainspec::ChainSpec;
use reth_node_api::{
    validate_version_specific_fields, EngineApiMessageVersion, EngineObjectValidationError,
    EngineTypes, EngineValidator, FullNodeTypes, NodeTypesWithEngine, PayloadOrAttributes,
};

use super::payload::StrataPayloadAttributes;
use crate::StrataEngineTypes;

/// Strata engine validator
#[derive(Debug, Clone)]
pub struct StrataEngineValidator {
    chain_spec: Arc<ChainSpec>,
}

impl<T> EngineValidator<T> for StrataEngineValidator
where
    T: EngineTypes<PayloadAttributes = StrataPayloadAttributes>,
{
    fn validate_version_specific_fields(
        &self,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, T::PayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(&self.chain_spec, version, payload_or_attrs)
    }

    fn ensure_well_formed_attributes(
        &self,
        version: EngineApiMessageVersion,
        attributes: &T::PayloadAttributes,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(&self.chain_spec, version, attributes.into())?;

        Ok(())
    }
}

/// Custom engine validator builder
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct StrataEngineValidatorBuilder;

impl<N> EngineValidatorBuilder<N> for StrataEngineValidatorBuilder
where
    N: FullNodeTypes<Types: NodeTypesWithEngine<Engine = StrataEngineTypes, ChainSpec = ChainSpec>>,
{
    type Validator = StrataEngineValidator;

    async fn build_validator(self, ctx: &BuilderContext<N>) -> eyre::Result<Self::Validator> {
        Ok(StrataEngineValidator {
            chain_spec: ctx.chain_spec(),
        })
    }
}
