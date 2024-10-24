use core::fmt;

use arbitrary::Arbitrary;
use bitcoin::BlockHash;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::buf::Buf32;

/// ID of an L1 block, usually the hash of its header.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Default,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Deserialize,
    Serialize,
)]
pub struct L1BlockId(Buf32);

impl L1BlockId {
    /// Computes the blkid from the header buf.  This expensive in proofs and
    /// should only be done when necessary.
    pub fn compute_from_header_buf(buf: &[u8]) -> L1BlockId {
        Self::from(strata_primitives::hash::sha256d(buf))
    }
}

impl From<Buf32> for L1BlockId {
    fn from(value: Buf32) -> Self {
        Self(value)
    }
}

impl From<BlockHash> for L1BlockId {
    fn from(value: BlockHash) -> Self {
        let value: Buf32 = value.into();
        value.into()
    }
}

impl AsRef<[u8; 32]> for L1BlockId {
    fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

// /// Implements same format as [`BlockHash`].
// /// Ignored for now because this is a breaking change!
// impl fmt::Debug for L1BlockId {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let reversed_bytes: Vec<u8> = self.0 .0.iter().rev().cloned().collect();
//         let mut buf = [0; 64];
//         hex::encode_to_slice(&reversed_bytes, &mut buf).expect("buf: enc hex");
//         f.write_str(unsafe { str::from_utf8_unchecked(&buf) })
//     }
// }

impl fmt::Debug for L1BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for L1BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::bitcoin::get_btc_mainnet_block;

    use super::L1BlockId;

    #[test]
    #[ignore = "breaking change"]
    fn test_l1_blkid() {
        let block = get_btc_mainnet_block();
        let l1_blkid: L1BlockId = block.block_hash().into();
        let str1 = format!("{}", block.block_hash());
        let str2 = format!("{:?}", l1_blkid);
        assert_eq!(str1, str2);
    }
}
