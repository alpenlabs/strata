pub fn hash_vec_to_string(hash_vec: Vec<u8>) -> String {
     hash_vec.iter().fold(String::new(),|acc,val| format!("{}{:02X}",acc, val))
}
