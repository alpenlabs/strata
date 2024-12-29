use bitcoin::{block::Header, consensus::deserialize};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::buf::Buf32;
use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use strata_state::{
    batch::BatchCheckpoint,
    l1::{get_btc_params, HeaderVerificationState, HeaderVerificationStateSnapshot},
    tx::DepositInfo,
};
use strata_zkvm::ZkVmEnv;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<BatchCheckpoint>,
    pub initial_snapshot: HeaderVerificationStateSnapshot,
    pub final_snapshot: HeaderVerificationStateSnapshot,
    pub rollup_params_commitment: Buf32,
}

impl L1BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

pub fn process_l1_batch_proof(zkvm: &impl ZkVmEnv, btc_blockspace_vk: &[u32; 8]) {
    let mut state: HeaderVerificationState = zkvm.read_borsh();

    let num_inputs: u32 = zkvm.read_serde();
    assert!(num_inputs > 0);

    let initial_snapshot = state.compute_initial_snapshot();
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;
    let mut rollup_params_commitment = None;

    for _ in 0..num_inputs {
        let blkpo: BlockspaceProofOutput = zkvm.read_verified_borsh(btc_blockspace_vk);
        let header: Header = deserialize(&blkpo.header_raw).unwrap();

        state.check_and_update_continuity(&header, &get_btc_params());
        deposits.extend(blkpo.deposits);
        prev_checkpoint = prev_checkpoint.or(blkpo.prev_checkpoint);

        // Ensure that the rollup parameters used are same for all blocks
        if let Some(filters_comm) = rollup_params_commitment {
            assert_eq!(blkpo.rollup_params_commitment, filters_comm);
        } else {
            rollup_params_commitment = Some(blkpo.rollup_params_commitment);
        }
    }
    let final_snapshot = state.compute_final_snapshot();

    let output = L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_snapshot,
        final_snapshot,
        rollup_params_commitment: rollup_params_commitment.unwrap(),
    };

    zkvm.commit_borsh(&output);
}
