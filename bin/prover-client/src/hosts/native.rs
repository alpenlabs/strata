use std::sync::Arc;

use strata_primitives::proof::ProofContext;
use strata_proofimpl_btc_blockspace::logic::process_blockscan_proof;
use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use strata_proofimpl_cl_stf::process_cl_stf;
use strata_proofimpl_evm_ee_stf::process_block_transaction_outer;
use zkaleido_native_adapter::{NativeHost, NativeMachine};

/// A mock verification key used in native mode when proof verification is not performed.
///
/// This constant provides a placeholder value for scenarios where a verification key is
/// required by a function signature, but actual verification is skipped.
const MOCK_VK: [u32; 8] = [0u32; 8];

/// Returns a reference to the appropriate [`NativeHost`] instance based on the given
/// [`ProofContext`].
///
/// This function maps the `ProofContext` variant to its corresponding [`NativeHost`] instance,
/// allowing for efficient host selection for different proof types.
pub fn get_host(id: &ProofContext) -> NativeHost {
    match id {
        ProofContext::BtcBlockspace(..) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_blockscan_proof(zkvm);
                Ok(())
            })),
        },
        ProofContext::EvmEeStf(..) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_block_transaction_outer(zkvm);
                Ok(())
            })),
        },
        ProofContext::ClStf(..) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_cl_stf(zkvm, &MOCK_VK, &MOCK_VK);
                Ok(())
            })),
        },
        ProofContext::Checkpoint(..) => NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_checkpoint_proof_outer(zkvm, &MOCK_VK);
                Ok(())
            })),
        },
    }
}
