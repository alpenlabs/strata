use digest::{generic_array::GenericArray, Digest};

pub type Hash = [u8; 32];

pub trait MerkleHasher {
    type Item;
    /// combines the left and Right nodes to form a single Node
    fn hash_node(left: Hash, right: Hash) -> Hash;
}

impl<D: Digest> MerkleHasher for D {
    type Item = Hash;

    fn hash_node(left: Hash, right: Hash) -> Hash {
        let mut context = D::new();
        context.update(left);
        context.update(right);

        let result: GenericArray<u8, D::OutputSize> = context.finalize();
        result.as_slice().try_into().unwrap()
    }
}
