use std::sync::Arc;

use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
use strata_primitives::proof::ProofId;
use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof_outer;
use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use strata_proofimpl_cl_agg::process_cl_agg;
use strata_proofimpl_cl_stf::process_cl_stf;
use strata_proofimpl_evm_ee_stf::process_block_transaction_outer;
use strata_proofimpl_l1_batch::process_l1_batch_proof;
use strata_zkvm::ZkVmHost;

/// A mock verification key used in native mode when proof verification is not performed.
///
/// This constant provides a placeholder value for scenarios where a verification key is
/// required by a function signature, but actual verification is skipped.
const MOCK_VK: [u32; 8] = [0u32; 8];

pub fn get_host(id: &ProofId) -> NativeHost {
    match id {
        ProofId::BtcBlockspace(_) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_blockspace_proof_outer(zkvm);
                Ok(())
            })),
        },
        ProofId::L1Batch(_, _) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_l1_batch_proof(zkvm, &MOCK_VK);
                Ok(())
            })),
        },
        ProofId::EvmEeStf(_) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_block_transaction_outer(zkvm);
                Ok(())
            })),
        },
        ProofId::ClStf(_) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_cl_stf(zkvm, &MOCK_VK);
                Ok(())
            })),
        },
        ProofId::ClAgg(_, _) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_cl_agg(zkvm, &MOCK_VK);
                Ok(())
            })),
        },
        ProofId::Checkpoint(_) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_checkpoint_proof_outer(zkvm, &MOCK_VK, &MOCK_VK);
                Ok(())
            })),
        },
    }
}
