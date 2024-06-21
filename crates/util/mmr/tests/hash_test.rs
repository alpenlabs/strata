#[cfg(test)]
mod test {
    use alpen_vertex_mmr::{hasher::MerkleHasher, utils::hash_vec_to_string};
    use sha2::{Digest, Sha256};

    #[test]
    fn check_hash_vec_to_string() {
        let mut hasher = Sha256::new();
        hasher.update("Block1");
        assert_eq!(hash_vec_to_string(hasher.finalize().to_vec()),
                   String::from("40E9B17A3391B5F461B2B96A2E5810A885F088346B901C65EBB5CF8CF7361103")
                   );
    }

    #[test]
    fn check_sha256_hash_chunk() {
        let chunk = b"TestChunk\0";
        println!("{:?}", chunk);

        let lhs: Vec<u8> = vec![219,33,218,85,49,43,59,231,163,181,164,100,128,165,138,200,159,221,117,151,132,143,225,180,184,110,231,198,249,0,231,254];
        assert_eq!(lhs, Sha256::hash_chunk(chunk.to_vec(), 0));
    }
        

    #[test]
    fn check_sha256_from_c_works() {
        // basically to check why C hash and Rust hash were not same turns out to be the \0 problem
        let mut hasher = Sha256::new();
        hasher.update(b"Hello\0");
        assert_eq!(hasher.finalize().to_vec(), vec![217,211,115,76,208,85,100,161,49,148,110,207,158,36,14,3,25,202,47,91,163,33,189,159,135,214,52,162,74,41,239,77]);
    }

}
