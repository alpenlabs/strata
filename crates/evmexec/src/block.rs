use borsh::BorshDeserialize;
use revm_primitives::B256;
use strata_primitives::evm_exec::EVMExtraPayload;
use strata_state::block::{L2Block, L2BlockBundle};
use thiserror::Error;

pub(crate) struct EVML2Block {
    #[allow(dead_code)]
    l2_block: L2Block,
    extra_payload: EVMExtraPayload,
}

impl EVML2Block {
    pub fn block_hash(&self) -> B256 {
        self.extra_payload.block_hash().into()
    }
}

impl TryFrom<L2BlockBundle> for EVML2Block {
    type Error = ConversionError;

    fn try_from(value: L2BlockBundle) -> Result<Self, Self::Error> {
        let extra_payload_slice = value.exec_segment().update().input().extra_payload();
        let extra_payload = EVMExtraPayload::try_from_slice(extra_payload_slice)
            .or(Err(ConversionError::Invalid))?;

        Ok(Self {
            l2_block: value.block().to_owned(),
            extra_payload,
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum ConversionError {
    #[error("Invalid EVM L2 Block")]
    Invalid,
}
