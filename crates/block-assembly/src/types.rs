use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf64, l2::L2BlockId};
use strata_state::{
    block::{L2Block, L2BlockAccessory, L2BlockBody, L2BlockBundle},
    header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SequencerDuty {
    SignBlock(u64),
    // SignCheckpoint(..)
    // ..
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTemplate {
    header: L2BlockHeader,
}

impl BlockTemplate {
    pub fn block_id(&self) -> L2BlockId {
        self.header.get_blockid()
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

    pub fn parent_block_id(&self) -> L2BlockId {
        self.parent_block_id
    }

    pub fn ts(&self, default: u64) -> u64 {
        self.ts.unwrap_or(default)
    }
}
