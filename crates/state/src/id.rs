use std::fmt;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;
use ssz_derive::{Decode, Encode};
use tree_hash::{Hash256, PackedEncoding, TreeHash, TreeHashType, HASHSIZE};
use tree_hash_derive::TreeHash;

/// ID of an L2 block, usually the hash of its root header.
#[derive(
    Copy,
    Clone,
    Eq,
    Default,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Arbitrary,
    BorshSerialize,
    BorshDeserialize,
    Decode,
    Encode,
)]
#[ssz(struct_behaviour = "transparent")]
pub struct L2BlockId(Buf32);

impl From<Buf32> for L2BlockId {
    fn from(value: Buf32) -> Self {
        Self(value)
    }
}

impl From<L2BlockId> for Buf32 {
    fn from(value: L2BlockId) -> Self {
        value.0
    }
}

impl AsRef<[u8; 32]> for L2BlockId {
    fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

impl fmt::Debug for L2BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for L2BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl TreeHash for L2BlockId {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        self.0.tree_hash_packed_encoding()
    }

    fn tree_hash_packing_factor() -> usize {
        HASHSIZE
    }

    #[allow(clippy::cast_lossless)] // Lint does not apply to all uses of this macro.
    fn tree_hash_root(&self) -> Hash256 {
        self.0.tree_hash_root()
    }
}
