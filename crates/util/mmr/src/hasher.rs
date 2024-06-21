use digest::Digest;

use crate::error::MerkleError;

pub type MerkleHash = [u8; 32];

const MERKLE_NODE_TAG : &str = "node";
const MERKLE_LEAF_TAG : &str = "leaf";

const MERKLE_CHUNK_SIZE : usize = 1024;

pub trait MerkleHasher {
    type Item; 
   
    /// Hashes a single chunk of data, based on the index. If the chunk index is beyond the range
    /// of buffer then it just returns the hash of a null chunk. I think we just need a variant of
    /// this function
    fn hash_chunk(chunk: Vec<u8>, chunk_index: usize) -> MerkleHash;

    /// simpler version of hash_chunk, where chunk index is supposed to be zero
    fn hash(chunk: Vec<u8>) -> Result<MerkleHash,MerkleError>;

    /// combines the left and Right nodes to form a single Node 
    fn hash_node(left: MerkleHash, right: MerkleHash) -> MerkleHash;
}

impl<D:Digest> MerkleHasher for D {
    type Item = MerkleHash;
    
    fn hash_chunk(chunk: Vec<u8>, chunk_index: usize) -> Self::Item{
        let mut context = D::new();
        let start_off = chunk_index * MERKLE_CHUNK_SIZE;
    
        if start_off >= chunk.len() {
            for _ in 0..(MERKLE_CHUNK_SIZE / 64) {
                context.update([0;64]);
            }
            context.update(MERKLE_LEAF_TAG.as_bytes());
            return context.finalize().to_vec().try_into().unwrap();
        }

        let mut buf_to_read = chunk.len() - start_off; 
        if buf_to_read > MERKLE_CHUNK_SIZE {
            buf_to_read = MERKLE_CHUNK_SIZE;
        }

        let mut bytes_remaining = MERKLE_CHUNK_SIZE;
        if buf_to_read > 0 {
            context.update(chunk.get(start_off..).expect("Out of Bounds"));
            bytes_remaining -= buf_to_read;
        }

        while bytes_remaining > 0 {
            context.update([0]) ;
            bytes_remaining -= 1;
        }
        context.update(MERKLE_LEAF_TAG.as_bytes());
        return context.finalize().to_vec().try_into().unwrap()
    }

    fn hash_node(left: MerkleHash, right: MerkleHash) -> MerkleHash{
        let mut context = D::new();
        context.update(left);
        context.update(right);

        context.update(MERKLE_NODE_TAG.as_bytes());

        context.finalize().to_vec().try_into().unwrap()
    }

    fn hash(chunk: Vec<u8>) -> Result<MerkleHash,MerkleError> {
        if chunk.len() > 1024 {
            return Err(MerkleError::ChunkSizeTooBig);
        }
        return Ok(Self::hash_chunk(chunk,0));
    }

}
