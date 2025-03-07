//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use strata_proofimpl_cl_stf::program::ClStfOutput;
use strata_state::batch::BatchTransition;
use zkaleido::ZkVmEnv;

pub mod program;

pub fn process_checkpoint_proof_outer(zkvm: &impl ZkVmEnv, cl_stf_vk: &[u32; 8]) {
    let batches_count: usize = zkvm.read_serde();
    assert!(batches_count > 0);

    let ClStfOutput {
        initial_chainstate_root,
        mut final_chainstate_root,
        mut initial_tx_filter_config_hash,
        mut final_tx_filter_config_hash,
    } = zkvm.read_verified_borsh(cl_stf_vk);

    // Starting with 1 since we have already read the first CL STF output
    for i in 1..batches_count {
        let cl_stf_output: ClStfOutput = zkvm.read_verified_borsh(cl_stf_vk);

        assert_eq!(
            cl_stf_output.initial_chainstate_root, final_chainstate_root,
            "continuity error"
        );

        final_chainstate_root = cl_stf_output.final_chainstate_root;

        // TxFilterConfig is updated in the terminal block of an epoch
        if i == batches_count - 1 {
            initial_tx_filter_config_hash = cl_stf_output.initial_tx_filter_config_hash;
            final_tx_filter_config_hash = cl_stf_output.final_tx_filter_config_hash;
        }
    }

    let output = BatchTransition {
        chainstate_transition: (initial_chainstate_root, final_chainstate_root),
        tx_filters_transition: (initial_tx_filter_config_hash, final_tx_filter_config_hash),
    };

    zkvm.commit_borsh(&output);
}
