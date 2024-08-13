use std::{
    io::{self, Cursor, Write},
    ops::Deref,
};

use alpen_express_primitives::buf::{Buf32, Buf64};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{id::L2BlockId, l1::L1BlockId};

/// Public parameters for batch proof to be posted to DA
/// will be updated as prover specs evolve
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BatchCommitment {
    // last safe L1 block for the batch
    l1blockid: L1BlockId,
    // last L2 block covered by the batch
    l2blockid: L2BlockId,
}

impl BatchCommitment {
    pub fn new(l1blockid: L1BlockId, l2blockid: L2BlockId) -> Self {
        Self {
            l1blockid,
            l2blockid,
        }
    }

    pub fn get_sighash(&self) -> Result<Buf32, io::Error> {
        let mut buf = [0; 32 + 32];

        let mut cur = Cursor::new(&mut buf[..]);
        cur.write_all(self.l1blockid.as_ref())?;
        cur.write_all(self.l2blockid.as_ref())?;

        Ok(alpen_express_primitives::hash::raw(&buf))
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SignedBatchCommitment {
    inner: BatchCommitment,
    signature: Buf64,
}

impl SignedBatchCommitment {
    pub fn new(inner: BatchCommitment, signature: Buf64) -> Self {
        Self { inner, signature }
    }
}

impl Deref for SignedBatchCommitment {
    type Target = BatchCommitment;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
