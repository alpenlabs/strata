use std::fs;

use strata_zkvm::{ProofSummary, ProofWithInfo, ZkVmHost};

use crate::provers::get_cache_dir;

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

fn get_summary<H: ZkVmHost>(hosts: Vec<&H>) {
    let summary: Vec<_> = hosts
        .into_iter()
        .map(|host| find_proof_and_extract_summary(format!("{}", host)))
        .collect();

    dbg!(summary);
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::provers::checkpoint;

    pub fn test_summary<H: ZkVmHost>(
        checkpoint_host: H,
        btc_host: H,
        l1_batch_host: H,
        el_host: H,
        cl_host: H,
        cl_agg_host: H,
    ) {
        checkpoint::test_proof(
            checkpoint_host.clone(),
            btc_host.clone(),
            l1_batch_host.clone(),
            el_host.clone(),
            cl_host.clone(),
            cl_agg_host.clone(),
        );

        get_summary(vec![
            &checkpoint_host,
            &btc_host,
            &l1_batch_host,
            &el_host,
            &cl_host,
            &cl_agg_host,
        ]);
    }

    #[test]
    #[cfg(not(any(feature = "risc0", feature = "sp1")))]
    fn test_native() {
        use crate::hosts::native::{
            btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch,
        };
        test_summary(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        use crate::hosts::risc0::{
            btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch,
        };
        test_summary(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        use crate::hosts::sp1::{btc_blockspace, checkpoint, cl_agg, cl_stf, evm_ee_stf, l1_batch};
        test_summary(
            checkpoint(),
            btc_blockspace(),
            l1_batch(),
            evm_ee_stf(),
            cl_stf(),
            cl_agg(),
        );
    }
}
