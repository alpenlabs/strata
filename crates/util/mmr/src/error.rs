use thiserror::Error;

#[derive(Error,Debug,PartialEq)]
pub enum MerkleError {
    #[error("No Elements present in MerkleTree")]
    NoElements,
    #[error("Not power of two")]
    NotPowerOfTwo,
    #[error("Index provided exceeds the bounds")]
    IndexOutOfBounds,
    #[error("Provided Chunk size is too Big")]
    ChunkSizeTooBig,
    #[error("Generic Error for unimplmented error")]
    Unknown,

}

