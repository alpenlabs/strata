#![allow(dead_code)]

//! Merkle mountain range implementation crate.
pub mod hasher;
pub mod error;
pub mod utils;


use error::MerkleError;
use sha2::Sha256;

use std::marker::PhantomData;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};


use hasher::{MerkleHasher, MerkleHash};


fn zero() -> MerkleHash {
    [0; 32]
}

fn is_zero(h: MerkleHash) -> bool {
    h.iter().all(|b| *b == 0)
}

/// Compact representation of the MMR that should be borsh serializable easily.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct CompactMmr {
    entries: u64,
    cap_log2: u8,
    roots: Vec<MerkleHash>,
}

/// Internal MMR state that can be easily updated.

#[derive(Clone)]
pub struct MerkleMr<H: MerkleHasher+Clone>{
    // number of elements inserted into mmr 
    pub num: u64,
    // Buffer of all possible peaks in mmr. only some of them will be valid at a time
    pub peaks: Vec<MerkleHash>,
    // maintained proof list that needs to get updated every time MerkleMr is updated 
    pub proof_list: Vec<MerkleProof<H>>,
    // phantom data for hasher 
    pub hasher: PhantomData<H>
}



impl<H: MerkleHasher+Clone> MerkleMr<H> {
    pub fn new() -> Self {
      Self {
           num: 0,
           peaks: Vec::new(),
           proof_list: vec![MerkleProof::new(0)],
           hasher: PhantomData 
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
            peaks: compact.roots.clone(),
            proof_list: Vec::new(),
            hasher: PhantomData
        }
    }


    pub fn to_compact(&self) -> CompactMmr {
        CompactMmr {
            entries: self.num,
            cap_log2: self.peaks.len() as u8,
            roots: self
                .peaks
                .iter()
                .filter(|h| is_zero(**h))
                .copied()
                .collect(),
        }
    }

    pub fn add_node(&mut self, hash_arr: MerkleHash) { 
        if self.num == 0 {
            self.peaks.push(hash_arr);
            self.num += 1;
            MerkleProof::<H>::new(1);
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
            // as this won't matter anyways
            self.peaks[current_height] = [0xff; 32];

            current_node =  next_node;
            current_height += 1;
        }

        if current_height >= self.peaks.len(){
            self.peaks.resize(current_height+1, [0;32]);
        }
        self.peaks[current_height] = current_node;
        self.num+=1;
    }

    pub fn get_single_root(&self) -> Result<MerkleHash, MerkleError> {
        if self.num == 0 {
            return Err(MerkleError::NoElements);
        }
        let mut i = 0;  
        while (1 << i) <= self.num {
            i+=1;
        }
        i-=1;

        // this only allows if there is one peak 
        // which means it should be in power of 2^0, 2^1, 2^2 and so on
        // this could be just self.num % 2
        if (1 << i) & self.num != self.num {
            return Err(MerkleError::NotPowerOfTwo);
        }

        Ok(self.peaks[i])
    }

    pub fn get_bagged_peak(&self) -> MerkleHash{
        format!("{:b}",self.num).chars().rev().enumerate().fold([0;32], |acc,val| {
            if val.1 == '1' {
                if acc == [0;32] {
                   return self.peaks[val.0];
                } else {
                    return H::hash_node(acc, self.peaks[val.0]);
                }
            }
            return acc;
        })
    }

    pub fn add_node_and_update_proof(&mut self, next: MerkleHash){
        if self.num == 0 {
            self.proof_list.push(MerkleProof::new(1));
            self.add_node(next);
            return;
        }

        let new_chunk_index = self.num;
        let peak_mask = self.num;
        let mut current_node = next;
        let mut current_height = 0;

        while (peak_mask >> current_height) & 1 == 1 {
            let prev_node = self.peaks[current_height];

            let next_node : MerkleHash = Sha256::hash_node(prev_node, current_node) ;

            let chunk_parent_tree = new_chunk_index >> (current_height + 1);
            for i in 0..self.proof_list.len() {
                let proof = &mut self.proof_list[i];
                let proof_index = proof.index;
                let proof_parent_tree = proof_index >> (current_height + 1);
                if chunk_parent_tree == proof_parent_tree {
                    if current_height >= proof.cohashes.len() {
                        proof.cohashes.resize(current_height+1, [0;32]);
                    }

                    if (proof_index >> current_height) & 1 == 1 {
                        proof.cohashes[current_height] = prev_node;
                        proof.prooflen+=1;
                    }else {
                        proof.cohashes[current_height] = current_node;
                        proof.prooflen+=1;
                    }
                }
            } 

            self.peaks[current_height] = [0xff;32];

            current_node = next_node;
            current_height += 1;
        }

        if current_height >= self.peaks.len(){
            self.peaks.resize(current_height+1, [0;32]);
        }
        self.peaks[current_height] = current_node;
        self.num+=1;
        self.proof_list.push(MerkleProof::new(self.num));
    } 
    /// generate_proof
    pub fn gen_proof(&self,index: usize) -> Result<MerkleProof<H>,MerkleError> {
        if index > self.num as usize{
            return Err(MerkleError::IndexOutOfBounds);
        }
        return Ok(self.proof_list[index].clone());
    }

    /// verifies if the hash of the element at particular index is present as a proof  
    pub fn verify(&self,proof: &MerkleProof<H>,hash: MerkleHash) -> Result<bool, MerkleError> {
        if proof.index as usize>= self.num as usize{
            return Err(MerkleError::IndexOutOfBounds);
        }

        return Ok(proof.proof_verify(self, hash));
    }

    /// constructs a MerkleProof and then verifies it
    pub fn verify_with_cohashes(&self, index: u64,hash: MerkleHash, cohashes: Vec<MerkleHash>) -> Result<bool, MerkleError> {
        let proof: MerkleProof<H> = MerkleProof {
            prooflen: cohashes.len(),
            cohashes,
            index,
            hasher: PhantomData,
        };

        self.verify(&proof, hash)
    }

}


#[derive(Debug,Clone)]
pub struct MerkleProof<H> where H: MerkleHasher + Clone {
    // sibling hashes required for proof
    pub cohashes: Vec<MerkleHash>,
    // length of proof
    pub prooflen: usize,
    // the index of the element for which this proof is for 
    pub index: u64,
    pub hasher: PhantomData<H>

}

impl<H: MerkleHasher+Clone> MerkleProof<H> {
    pub fn new(index: u64) -> Self {
        Self {
            cohashes : vec![[0;32]],
            prooflen: 0,
            index,
            hasher: PhantomData
        }
    }
    /// builds the new MerkleProof from the provided Data 
    pub fn from_cohashes(cohashes: Vec<MerkleHash>,index: u64) -> Self {
        Self {
            prooflen: cohashes.len(),
            cohashes,
            index,
            hasher: PhantomData
        }
    }

    /// verifies the hash against the current proof for given mmr
    pub fn proof_verify(&self, mmr: &MerkleMr<H>, chunk_hash: MerkleHash) -> bool {
        let root = mmr.peaks[self.prooflen];
        if self.prooflen == 0 {
            return root == chunk_hash;
        }
        let mut cur_hash = chunk_hash;
        let mut side_flags = self.index;

        for i in 0..self.prooflen {
            let node_hash; 
            if side_flags & 1 == 1{
                node_hash = Sha256::hash_node(self.cohashes[i], cur_hash);
            } else {
                node_hash = Sha256::hash_node(cur_hash, self.cohashes[i]);
            }

            side_flags >>= 1;
            cur_hash = node_hash;
        }
        return cur_hash == root;
    }
}






