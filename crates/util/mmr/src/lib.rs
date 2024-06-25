//! Merkle mountain range implementation crate.
pub mod error;
pub mod hasher;

use std::marker::PhantomData;

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use error::MerkleError;
use hasher::{Hash, MerkleHasher};

fn zero() -> Hash {
    [0; 32]
}

fn is_zero(h: Hash) -> bool {
    h.iter().all(|b| *b == 0)
}

/// Compact representation of the MMR that should be borsh serializable easily.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct CompactMmr {
    entries: u64,
    cap_log2: u8,
    roots: Vec<Hash>,
}

#[derive(Clone)]
pub struct MerkleMr<H: MerkleHasher + Clone> {
    // number of elements inserted into mmr
    pub num: u64,
    // Buffer of all possible peaks in mmr. only some of them will be valid at a time
    pub peaks: Vec<Hash>,
    // phantom data for hasher
    pub hasher: PhantomData<H>,
}

impl<H: MerkleHasher + Clone> MerkleMr<H> {
    pub fn new(cap_log2: usize) -> Self {
        Self {
            num: 0,
            peaks: Vec::with_capacity(cap_log2),
            hasher: PhantomData,
        }
    }

    pub fn from_compact(compact: &CompactMmr) -> Self {
        // FIXME this is somewhat inefficient, we could consume the vec and just
        // slice out its elements, but this is fine for now
        let mut roots = vec![zero(); compact.cap_log2 as usize];
        let mut at = 0;
        for i in 0..compact.cap_log2 {
            if compact.entries >> i & 1 != 0 {
                roots[i as usize] = compact.roots[at as usize];
                at += 1;
            }
        }

        Self {
            num: compact.entries,
            peaks: roots,
            hasher: PhantomData,
        }
    }

    pub fn to_compact(&self) -> CompactMmr {
        CompactMmr {
            entries: self.num,
            cap_log2: self.peaks.len() as u8,
            roots: self
                .peaks
                .iter()
                .filter(|h| !is_zero(**h))
                .copied()
                .collect(),
        }
    }

    pub fn add_leaf(&mut self, hash_arr: Hash) {
        if self.num == 0 {
            self.peaks.push(hash_arr);
            self.num += 1;
            return;
        }

        // the number of elements in MMR is also the mask of peaks
        let peak_mask = self.num;

        let mut current_node = hash_arr;
        // we iterate through the height
        let mut current_height = 0;

        while (peak_mask >> current_height) & 1 == 1 {
            let next_node = H::hash_node(self.peaks[current_height], current_node);

            // setting this for debugging purpose
            self.peaks[current_height] = [0; 32];

            current_node = next_node;
            current_height += 1;
        }

        if current_height >= self.peaks.len() {
            self.peaks.resize(current_height + 1, [0; 32]);
        }
        self.peaks[current_height] = current_node;
        self.num += 1;
    }

    pub fn get_single_root(&self) -> Result<Hash, MerkleError> {
        println!("{}", self.num);
        println!("{}", self.num.is_power_of_two());
        if self.num == 0 {
            return Err(MerkleError::NoElements);
        }
        if !self.num.is_power_of_two() && self.num != 1 {
            return Err(MerkleError::NotPowerOfTwo);
        }

        Ok(self.peaks[(self.num.ilog2()) as usize])
    }
    pub fn add_leaf_updating_proof(&mut self, next: Hash, proof: &mut MerkleProof<H>) {
        if self.num == 0 {
            self.add_leaf(next);
            *proof = MerkleProof::new(self.num);
            return;
        }

        let new_leaf_index = self.num;
        let peak_mask = self.num;
        let mut current_node = next;
        let mut current_height = 0;
        while (peak_mask >> current_height) & 1 == 1 {
            let prev_node = self.peaks[current_height];
            let next_node: Hash = H::hash_node(prev_node, current_node);
            let leaf_parent_tree = new_leaf_index >> (current_height + 1);
            self.update_single_proof(
                proof,
                leaf_parent_tree,
                current_height,
                prev_node,
                current_node,
            );

            self.peaks[current_height] = [0; 32];
            current_node = next_node;
            current_height += 1;
        }

        if current_height >= self.peaks.len() {
            self.peaks.resize(current_height + 1, [0; 32]);
        }
        self.peaks[current_height] = current_node;
        self.num += 1;
    }

    fn update_single_proof(
        &mut self,
        proof: &mut MerkleProof<H>,
        leaf_parent_tree: u64,
        current_height: usize,
        prev_node: Hash,
        current_node: Hash,
    ) {
        let proof_index = proof.index;
        let proof_parent_tree = proof_index >> (current_height + 1);
        if leaf_parent_tree == proof_parent_tree {
            if current_height >= proof.cohashes.len() {
                proof.cohashes.resize(current_height + 1, [0; 32]);
            }
            if (proof_index >> current_height) & 1 == 1 {
                proof.cohashes[current_height] = prev_node;
                proof.prooflen += 1;
            } else {
                proof.cohashes[current_height] = current_node;
                proof.prooflen += 1;
            }
        }
    }

    pub fn add_leaf_updating_proof_list(
        &mut self,
        next: Hash,
        proof_list: &mut Vec<MerkleProof<H>>,
    ) {
        // the proof list assumes that from the position you start maintaining proof, all other
        // subsequent proofs for next elements are also maintained so we create proof ready to be
        // worked on next pass
        if proof_list.is_empty() {
            proof_list.push(MerkleProof::new(self.num));
            proof_list.push(MerkleProof::new(self.num + 1));
        } else {
            proof_list.push(MerkleProof::new(self.num + 1));
        }

        if self.num == 0 {
            self.add_leaf(next);
            return;
        }

        let new_leaf_index = self.num;
        let peak_mask = self.num;
        let mut current_node = next;
        let mut current_height = 0;
        while (peak_mask >> current_height) & 1 == 1 {
            let prev_node = self.peaks[current_height];
            let next_node: Hash = H::hash_node(prev_node, current_node);
            let leaf_parent_tree = new_leaf_index >> (current_height + 1);

            for proof in proof_list.iter_mut() {
                self.update_single_proof(
                    proof,
                    leaf_parent_tree,
                    current_height,
                    prev_node,
                    current_node,
                );
            }

            self.peaks[current_height] = [0; 32];
            current_node = next_node;
            current_height += 1;
        }

        if current_height >= self.peaks.len() {
            self.peaks.resize(current_height + 1, [0; 32]);
        }
        self.peaks[current_height] = current_node;
        self.num += 1;
    }

      pub fn verify(&self, cohashes: &[Hash],leaf_index: u64, leaf_hash: Hash) -> bool {
        let root = self.peaks[cohashes.len()];
        if cohashes.len() == 0 {
            return root == leaf_hash;
        }

        let mut cur_hash = leaf_hash;
        let mut side_flags = leaf_index;

        for i in 0..cohashes.len(){
            let node_hash = if side_flags & 1 == 1 {
                H::hash_node(cohashes[i], cur_hash)
            } else {
                H::hash_node(cur_hash, cohashes[i])
            };

            side_flags >>= 1;
            cur_hash = node_hash;
        }
        cur_hash == root
    }


    pub fn gen_proof(
        &self,
        proof_list: &[MerkleProof<H>],
        index: usize,
    ) -> Result<MerkleProof<H>, MerkleError> {
        if index > self.num as usize {
            return Err(MerkleError::IndexOutOfBounds);
        }
        Ok(proof_list[index].clone())
    }
}

#[derive(Debug, Clone)]
pub struct MerkleProof<H>
where
    H: MerkleHasher + Clone,
{
    // sibling hashes required for proof
    pub cohashes: Vec<Hash>,
    // length of proof
    pub prooflen: usize,
    // the index of the element for which this proof is for
    pub index: u64,
    pub _pd: PhantomData<H>,
}

impl<H: MerkleHasher + Clone> MerkleProof<H> {
    pub fn new(index: u64) -> Self {
        Self {
            cohashes: vec![[0; 32]],
            prooflen: 0,
            index,
            _pd: PhantomData,
        }
    }

    /// builds the new MerkleProof from the provided Data
    pub fn from_cohashes(cohashes: Vec<Hash>, index: u64) -> Self {
        Self {
            prooflen: cohashes.len(),
            cohashes,
            index,
            _pd: PhantomData,
        }
    }

    /// verifies the hash against the current proof for given mmr
    pub fn proof_verify(&self, mmr: &MerkleMr<H>, leaf_hash: Hash) -> bool {
        let root = mmr.peaks[self.prooflen];
        if self.prooflen == 0 {
            return root == leaf_hash;
        }
        let mut cur_hash = leaf_hash;
        let mut side_flags = self.index;

        for i in 0..self.prooflen {
            let node_hash = if side_flags & 1 == 1 {
                H::hash_node(self.cohashes[i], cur_hash)
            } else {
                H::hash_node(cur_hash, self.cohashes[i])
            };

            side_flags >>= 1;
            cur_hash = node_hash;
        }
        cur_hash == root
    }
}

#[cfg(test)]
mod test {
    use sha2::Sha256;

    use crate::error::MerkleError;

    use super::{
        hasher::{Hash, MerkleHasher},
        MerkleMr, MerkleProof,
    };

    fn generate_for_n_integers(n: usize) -> (MerkleMr<Sha256>, Vec<MerkleProof<Sha256>>) {
        let mut mmr: MerkleMr<Sha256> = MerkleMr::new(1);

        let mut proof = Vec::new();
        let list_of_hashes = generate_hashes_for_n_integers(n);

        (0..n).for_each(|i| mmr.add_leaf_updating_proof_list(list_of_hashes[i], &mut proof));
        (mmr, proof)
    }

    fn generate_hashes_for_n_integers(n: usize) -> Vec<Hash> {
        (0..n)
            .map(|i| Sha256::hash_leaves(i.to_be_bytes().to_vec()))
            .collect::<Vec<Hash>>()
    }

    fn mmr_proof_for_specific_nodes(n: usize, specific_nodes: Vec<usize>) {
        //[0]
        let (mmr, proof_list) = generate_for_n_integers(n);

        let proof: Vec<MerkleProof<Sha256>> = specific_nodes
            .iter()
            .map(|i| {
                mmr.gen_proof(&proof_list, if *i == 0 { *i } else { *i - 1 })
                    .expect("cannot generate proof")
            })
            .collect();
        let hash: Vec<Hash> = specific_nodes
            .iter()
            .map(|i| Sha256::hash_leaves(i.to_be_bytes().to_vec()))
            .collect();

        (0..specific_nodes.len()).for_each(|i| {
            assert!(mmr.verify(&proof[i].cohashes,proof[i].index,hash[i]));
        });
    }

    #[test]
    fn check_zero_elements() {
        mmr_proof_for_specific_nodes(0, vec![]);
    }

    #[test]
    fn check_two_sibling_leaves() {
        mmr_proof_for_specific_nodes(11, vec![4, 5]);
        mmr_proof_for_specific_nodes(11, vec![5, 6]);
    }

    #[test]
    fn check_single_element() {
        //[0]
        let (mmr, proof_list) = generate_for_n_integers(1);

        let proof = mmr
            .gen_proof(&proof_list, 0)
            .expect("cannot generate proof");

        let hash = Sha256::hash_leaves(0_usize.to_be_bytes().to_vec());
        assert!(mmr.verify(&proof.cohashes,proof.index, hash));
    }

    #[test]
    fn check_two_peaks() {
        mmr_proof_for_specific_nodes(3, vec![0, 2]);
    }

    #[test]
    fn check_five_hundred_elements() {
        mmr_proof_for_specific_nodes(500, vec![0, 456]);
    }

    #[test]
    fn check_peak_for_mmr_single_leaf() {
        let hashed1 = Sha256::hash_leaves(b"first".to_vec());

        let mut mmr: MerkleMr<Sha256> = MerkleMr::new(2);
        mmr.add_leaf(hashed1.try_into().unwrap());

        assert_eq!(
            mmr.get_single_root(),
            Ok([
                167, 147, 123, 100, 184, 202, 165, 143, 3, 114, 27, 182, 186, 207, 92, 120, 203,
                35, 95, 235, 224, 231, 11, 27, 132, 205, 153, 84, 20, 97, 160, 142
            ])
        );
    }

    #[test]
    fn check_peak_for_mmr_three_leaves() {
        let hashed1 = Sha256::hash_leaves(b"first".to_vec());

        let mut mmr: MerkleMr<Sha256> = MerkleMr::new(1);
        mmr.add_leaf(hashed1.try_into().unwrap());
        mmr.add_leaf(hashed1.try_into().unwrap());
        mmr.add_leaf(hashed1.try_into().unwrap());

        assert_eq!(mmr.get_single_root(), Err(MerkleError::NotPowerOfTwo));
    }

    #[test]
    fn check_peak_for_mmr_four_leaves() {
        let hashed1 = Sha256::hash_leaves(b"first".to_vec());

        let mut mmr: MerkleMr<Sha256> = MerkleMr::new(1);
        mmr.add_leaf(hashed1.try_into().unwrap());
        mmr.add_leaf(hashed1.try_into().unwrap());
        mmr.add_leaf(hashed1.try_into().unwrap());
        mmr.add_leaf(hashed1.try_into().unwrap());

        assert_eq!(
            mmr.get_single_root(),
            Ok([
                42, 45, 97, 143, 48, 40, 235, 23, 80, 22, 226, 97, 57, 191, 239, 146, 157, 81, 89,
                225, 228, 51, 162, 223, 102, 47, 76, 12, 171, 93, 173, 96
            ])
        );
    }

    #[test]
    fn check_invalid_proof() {
        let (mmr, _) = generate_for_n_integers(5);
        let invalid_proof = MerkleProof::<Sha256>::new(6);
        let hash = Sha256::hash_leaves(6_usize.to_be_bytes().to_vec());

        assert!(matches!(
            mmr.verify(&invalid_proof.cohashes,0, hash),
            false 
        ));
    }

    #[test]
    fn check_add_node_and_update() {
        let mut mmr: MerkleMr<Sha256> = MerkleMr::new(3);
        let mut proof_list = Vec::new();

        let hashed0: Hash = Sha256::hash_leaves(b"first".to_vec());
        let hashed1: Hash = Sha256::hash_leaves(b"second".to_vec());
        let hashed2: Hash = Sha256::hash_leaves(b"third".to_vec());
        let hashed3: Hash = Sha256::hash_leaves(b"fourth".to_vec());
        let hashed4: Hash = Sha256::hash_leaves(b"fifth".to_vec());

        mmr.add_leaf_updating_proof_list(hashed0, &mut proof_list);
        mmr.add_leaf_updating_proof_list(hashed1, &mut proof_list);
        mmr.add_leaf_updating_proof_list(hashed2, &mut proof_list);
        mmr.add_leaf_updating_proof_list(hashed3, &mut proof_list);
        mmr.add_leaf_updating_proof_list(hashed4, &mut proof_list);

        assert!(proof_list[0].proof_verify(&mmr, hashed0));
        assert!(proof_list[1].proof_verify(&mmr, hashed1));
        assert!(proof_list[2].proof_verify(&mmr, hashed2));
        assert!(proof_list[3].proof_verify(&mmr, hashed3));
        assert!(proof_list[4].proof_verify(&mmr, hashed4));
    }

    #[test]
    fn check_compact_and_non_compact() {
        let (mmr, _) = generate_for_n_integers(5);

        let compact_mmr = mmr.to_compact();
        let deserialized_mmr = MerkleMr::<Sha256>::from_compact(&compact_mmr);

        assert_eq!(mmr.num, deserialized_mmr.num);
        assert_eq!(mmr.peaks, deserialized_mmr.peaks);
    }

    #[test]
    fn arbitrary_index_proof() {
        let (mut mmr, _) = generate_for_n_integers(20);
        // update proof for 21st element
        let mut proof: MerkleProof<Sha256> = MerkleProof::new(20);

        // add 4 elements into mmr  so 20 + 4 elements
        let elem = 4;
        let num_hash = generate_hashes_for_n_integers(elem);

        for i in 0..elem {
            mmr.add_leaf_updating_proof(num_hash[i], &mut proof);
        }

        assert!(proof.proof_verify(&mmr, num_hash[0].try_into().unwrap()));
    }

    #[test]
    fn update_proof_list_from_arbitrary_index() {
        let (mut mmr, _) = generate_for_n_integers(20);
        // update proof for 21st element
        let mut proof_list = Vec::new();

        // add 4 elements into mmr  so 20 + 4 elements
        let elem = 4;
        let num_hash = generate_hashes_for_n_integers(elem);

        for i in 0..elem {
            mmr.add_leaf_updating_proof_list(num_hash[i], &mut proof_list);
        }

        for i in 0..elem {
            assert!(proof_list[i].proof_verify(&mmr, num_hash[i].try_into().unwrap()));
        }
    }
}
