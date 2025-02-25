use zkaleido::ZkVmEnv;

mod prover;
pub use prover::*;

pub fn process_cl_agg(zkvm: &impl ZkVmEnv, _cl_stf_vk: &[u32; 8]) {
    let num_agg_inputs: u32 = zkvm.read_serde();
    assert!(
        num_agg_inputs >= 1,
        "At least one CL proof is required for aggregation"
    );

    // FIXME: this crate can be deleted completely

    zkvm.commit_borsh(&2);
}
