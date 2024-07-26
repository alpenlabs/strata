use borsh::BorshDeserialize;
use reth_primitives::B256;
use thiserror::Error;

use alpen_vertex_primitives::evm_exec::EVMExtraPayload;
use alpen_vertex_state::block::L2Block;

pub(crate) struct EVML2Block {
    l2_block: L2Block,
    extra_payload: EVMExtraPayload,
}

impl EVML2Block {
    pub fn block_hash(&self) -> B256 {
        self.extra_payload.block_hash().into()
    }
}

impl TryFrom<L2Block> for EVML2Block {
    type Error = ConversionError;

    fn try_from(value: L2Block) -> Result<Self, Self::Error> {
        let extra_payload_slice = value.exec_segment().update().input().extra_payload();
        let extra_payload = EVMExtraPayload::try_from_slice(extra_payload_slice)
            .or(Err(ConversionError::Invalid))?;

        Ok(Self {
            l2_block: value,
            extra_payload,
        })
    }
}

#[derive(Debug, Error)]
pub(crate) enum ConversionError {
    #[error("Invalid EVM L2 Block")]
    Invalid,
}
