use digest::generic_array::GenericArray;
use digest::Digest;

pub type Hash = [u8; 32];

pub trait MerkleHasher {
    type Item;
    /// Hashes a data to be stored into leaf
    fn hash_leaves(leaf: Vec<u8>) -> Hash;
    /// combines the left and Right nodes to form a single Node
    fn hash_node(left: Hash, right: Hash) -> Hash;
}

impl<D: Digest> MerkleHasher for D {
    type Item = Hash;

    fn hash_leaves(leaf: Vec<u8>) -> Self::Item {
        let mut context = D::new();
        context.update(leaf);

        let result: GenericArray<u8, D::OutputSize> = context.finalize();
        result.as_slice().try_into().unwrap()
    }

    fn hash_node(left: Hash, right: Hash) -> Hash {
        let mut context = D::new();
        context.update(left);
        context.update(right);

        let result: GenericArray<u8, D::OutputSize> = context.finalize();
        result.as_slice().try_into().unwrap()
    }
}
