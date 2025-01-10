use serde::{Deserialize, Serialize};
#[cfg(feature = "debug-utils")]
use strata_common::bail_manager::{check_bail_trigger, BAIL_DUTY_SIGN_BLOCK};
use strata_primitives::{buf::Buf64, l2::L2BlockId};
use strata_state::{
    block::{L2Block, L2BlockAccessory, L2BlockBody, L2BlockBundle},
    header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
};

/// Block template with header, body, and accessory.
#[derive(Debug, Clone)]
pub struct BlockTemplateFull {
    header: L2BlockHeader,
    body: L2BlockBody,
    accessory: L2BlockAccessory,
}

impl BlockTemplateFull {
    pub fn new(header: L2BlockHeader, body: L2BlockBody, accessory: L2BlockAccessory) -> Self {
        Self {
            header,
            body,
            accessory,
        }
    }

    pub fn block_id(&self) -> L2BlockId {
        self.header.get_blockid()
    }

    pub fn header(&self) -> &L2BlockHeader {
        &self.header
    }

    pub fn complete_block_template(self, completion: BlockCompletionData) -> L2BlockBundle {
        #[cfg(feature = "debug-utils")]
        check_bail_trigger(BAIL_DUTY_SIGN_BLOCK);
        let BlockTemplateFull {
            header,
            body,
            accessory,
        } = self;
        let BlockCompletionData { signature } = completion;
        let signed_header = SignedL2BlockHeader::new(header, signature);

        let block = L2Block::new(signed_header, body);
        L2BlockBundle::new(block, accessory)
    }
}

/// Block template with only sufficient info for signing to be passed for signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTemplate {
    header: L2BlockHeader,
}

impl BlockTemplate {
    pub fn template_id(&self) -> L2BlockId {
        self.header.get_blockid()
    }

    pub fn header(&self) -> &L2BlockHeader {
        &self.header
    }

    pub fn from_full_ref(full: &BlockTemplateFull) -> Self {
        Self {
            header: full.header.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCompletionData {
    signature: Buf64,
}

impl BlockCompletionData {
    pub fn from_signature(signature: Buf64) -> Self {
        Self { signature }
    }

    pub fn signature(&self) -> &Buf64 {
        &self.signature
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BlockGenerationConfig {
    parent_block_id: L2BlockId,
    #[serde(skip_serializing_if = "Option::is_none")]
    ts: Option<u64>,
    // slot: Option<u64>,
    // el payload ?
}

impl BlockGenerationConfig {
    pub fn from_parent_block_id(parent_block_id: L2BlockId) -> Self {
        Self {
            parent_block_id,
            ..Default::default()
        }
    }

    pub fn with_ts(mut self, ts: u64) -> Self {
        self.ts = Some(ts);
        self
    }

    pub fn parent_block_id(&self) -> L2BlockId {
        self.parent_block_id
    }

    pub fn ts(&self, default: u64) -> u64 {
        self.ts.unwrap_or(default)
    }
}
