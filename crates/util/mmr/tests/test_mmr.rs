#[cfg(test)]
mod test {
use alpen_vertex_mmr::{hasher::{MerkleHash, MerkleHasher}, MerkleMr, MerkleProof};
use sha2::Sha256;


fn generate_for_n_integers(n: usize) -> MerkleMr<Sha256>{
    let mut mmr: MerkleMr<Sha256> = MerkleMr::new();

    let list_of_hashes = generate_hashes_for_n_integers(n);

    (0..n).for_each(|i| {
        mmr.add_node_and_update_proof(list_of_hashes[i])
    });
    mmr
}

fn generate_hashes_for_n_integers(n:usize) -> Vec<MerkleHash> {
    (0..n).map(|i| Sha256::hash(i.to_be_bytes().to_vec()).unwrap()).collect::<Vec<MerkleHash>>()
}

fn mmr_proof_for_specific_nodes(n: usize,specific_nodes: Vec<usize>) {
    //[0]
    let mmr = generate_for_n_integers(n);

    let proof: Vec<MerkleProof<Sha256>> = specific_nodes.iter().map(|i| mmr.gen_proof(if *i == 0 { *i } else {*i - 1 }).expect("cannot generate proof")).collect();
    println!("{}",mmr.num);
    let hash: Vec<MerkleHash> = specific_nodes.iter().map(|i| Sha256::hash(i.to_be_bytes().to_vec()).unwrap()).collect();
    
    (0..specific_nodes.len()).for_each(|i| {
        assert!(mmr.verify(&proof[i], hash[i]).is_ok());
    });


    // assert!(mmr.verify(proof, hash).is_ok());
    // assert!(mmr.verify(proof, hash).expect("merkle error"));
}




#[test]
fn check_single_element() {
    //[0]
    let mmr = generate_for_n_integers(1);

    let proof = mmr.gen_proof(0).expect("cannot generate proof");
    let hash = Sha256::hash(0_usize.to_be_bytes().to_vec()).expect("cannot generate hash");

    // assert!(mmr.verify(proof, hash).is_ok());
    assert!(mmr.verify(&proof, hash).expect("merkle error"));
} 

#[test]
fn check_two_peaks() {
    mmr_proof_for_specific_nodes(3, vec![0,2]);
}
#[test]
fn check_500_elements() {
    mmr_proof_for_specific_nodes(500, vec![0,456]);
} 

#[test]
fn check_zero_elements() {
    mmr_proof_for_specific_nodes(0, vec![]);
}

#[test]
fn check_peak_for_mmr_single_chunk() {
    let hashed1 = Sha256::hash_chunk(b"first\0".to_vec(), 0);
    
    let mut mmr:MerkleMr<Sha256> = MerkleMr::new();
    mmr.add_node(hashed1.try_into().unwrap());

    assert_eq!(mmr.get_single_root(), Ok([160,172,79,73,139,166,210,112,135,115,46,46,19,163,132,101,32,73,190,166,31,207,194,202,95,116,120,4,58,157,149,225]));
} 

#[test]
fn check_peak_for_mmr_two_chunks() {

    let hashed1 = Sha256::hash_chunk(b"first\0".to_vec(), 0);
    let hashed2 = Sha256::hash_chunk(b"second\0".to_vec(), 0);
    
    let mut mmr : MerkleMr<Sha256> = MerkleMr::new();
    mmr.add_node(hashed1.try_into().unwrap());
    mmr.add_node(hashed2.try_into().unwrap());

    assert_eq!(mmr.get_single_root(), Ok([178,84,15,86,221,21,214,5,210,160,49,11,59,8,162,5,151,129,255,203,207,205,78,192,15,5,82,3,191,82,38,253]));

}


#[test]
fn check_hash_node() {
    let left = Sha256::hash_chunk(b"first\0".to_vec(), 0);
    let right = Sha256::hash_chunk(b"second\0".to_vec(), 0);

    assert_eq!([178,84,15,86,221,21,214,5,210,160,49,11,59,8,162,5,151,129,255,203,207,205,78,192,15,5,82,3,191,82,38,253], Sha256::hash_node(left.try_into().unwrap(), right.try_into().unwrap()));
} 

#[test]
fn check_peaks_for_five_dummy_strings() {
    let mmr = create_dummy_mmr();

    let peaks_state = [
        [ 207,255,173,209,110,82,111,179,172,93,113,52,32,214,5,160,201,69,124,119,60,87,159,190,237,171,200,193,205,248,17,183],
        [0xff;32],
        [ 90,125,237,3,151,189,117,254,61,72,166,100,141,35,87,184,204,211,86,40,202,11,52,92,4,28,121,154,133,166,89,190]
    ];

    for i in 0..mmr.peaks.len() {
        assert_eq!(peaks_state[i], mmr.peaks[i]);
    }
}

#[test]
fn check_bagged_peak() {
    let hashed1 = Sha256::hash_chunk(b"first\0".to_vec(), 0);
    let hashed2 = Sha256::hash_chunk(b"second\0".to_vec(), 0);
    
    let mut mmr : MerkleMr<Sha256> = MerkleMr::new();
    mmr.add_node(hashed1.try_into().unwrap());
    mmr.add_node(hashed2.try_into().unwrap());
    
    assert_eq!(mmr.get_bagged_peak(), [178, 84, 15, 86, 221, 21, 214, 5, 210, 160, 49, 11, 59, 8, 162, 5, 151, 129, 255, 203, 207, 205, 78, 192, 15, 5, 82, 3, 191 , 82, 38,253]);
}


#[test]
fn check_add_node_and_update() {
    let mut mmr : MerkleMr<Sha256> = MerkleMr::new(); 

    let hashed0: MerkleHash = Sha256::hash_chunk(b"first\0".to_vec(), 0).try_into().unwrap();
    let hashed1: MerkleHash = Sha256::hash_chunk(b"second\0".to_vec(), 0).try_into().unwrap();
    let hashed2: MerkleHash = Sha256::hash_chunk(b"third\0".to_vec(), 0).try_into().unwrap();
    let hashed3: MerkleHash = Sha256::hash_chunk(b"fourth\0".to_vec(), 0).try_into().unwrap();
    let hashed4: MerkleHash = Sha256::hash_chunk(b"fifth\0".to_vec(), 0).try_into().unwrap();

    mmr.add_node_and_update_proof(hashed0);
    mmr.add_node_and_update_proof(hashed1);
    mmr.add_node_and_update_proof(hashed2);
    mmr.add_node_and_update_proof(hashed3);
    mmr.add_node_and_update_proof(hashed4);
    
    assert!(mmr.proof_list[0].proof_verify(&mmr,hashed0));
    assert!(mmr.proof_list[1].proof_verify(&mmr,hashed1));
    assert!(mmr.proof_list[2].proof_verify(&mmr,hashed2));
    assert!(mmr.proof_list[3].proof_verify(&mmr,hashed3));
    assert!(mmr.proof_list[4].proof_verify(&mmr,hashed4));

    // println!("{:?}", mmr.proof_list);
}

fn create_dummy_mmr() -> MerkleMr<Sha256> {
    let hashed1 = Sha256::hash_chunk(b"first\0".to_vec(), 0);
    let hashed2 = Sha256::hash_chunk(b"second\0".to_vec(), 0);
    let hashed3 = Sha256::hash_chunk(b"third\0".to_vec(), 0);
    let hashed4 = Sha256::hash_chunk(b"fourth\0".to_vec(), 0);
    let hashed5 = Sha256::hash_chunk(b"fifth\0".to_vec(), 0);

    let mut mmr = MerkleMr::new();

    mmr.add_node(hashed1.try_into().unwrap());
    mmr.add_node(hashed2.try_into().unwrap());
    mmr.add_node(hashed3.try_into().unwrap());
    mmr.add_node(hashed4.try_into().unwrap());
    mmr.add_node(hashed5.try_into().unwrap());

    mmr
}


}
