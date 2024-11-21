use std::fs;

use strata_zkvm::{ProofSummary, ProofWithInfo, ZkVmHost};

use crate::proof_generator::get_cache_dir;

fn find_proof_and_extract_summary(host: String) -> ProofSummary {
    let entries = fs::read_dir(get_cache_dir()).expect("Failed to read cache directory");

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                if file_name.contains(&host) {
                    let proof = ProofWithInfo::load(path).expect("Failed to load proof file");
                    return proof.into();
                }
            }
        }
    }

    panic!("No proof file found containing '{}'", host);
}

fn get_summary<H: ZkVmHost>(hosts: Vec<H>) {
    let summary: Vec<_> = hosts
        .into_iter()
        .map(|host| find_proof_and_extract_summary(format!("{}", host)))
        .collect();

    dbg!(summary);
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{btc, checkpoint, cl, el, l1_batch, l2_batch};

    #[test]
    fn test_native() {
        get_summary(vec![
            checkpoint::get_native_host(),
            btc::get_native_host(),
            l1_batch::get_native_host(),
            el::get_native_host(),
            cl::get_native_host(),
            l2_batch::get_native_host(),
        ]);
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        get_summary(vec![
            checkpoint::get_risc0_host(),
            btc::get_risc0_host(),
            l1_batch::get_risc0_host(),
            el::get_risc0_host(),
            cl::get_risc0_host(),
            l2_batch::get_risc0_host(),
        ]);
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        get_summary(vec![
            checkpoint::get_sp1_host(),
            btc::get_sp1_host(),
            l1_batch::get_sp1_host(),
            el::get_sp1_host(),
            cl::get_sp1_host(),
            l2_batch::get_sp1_host(),
        ]);
    }
}
