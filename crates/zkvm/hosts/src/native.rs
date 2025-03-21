use std::sync::{Arc, LazyLock};

use strata_proofimpl_btc_blockspace::logic::process_blockscan_proof;
use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use strata_proofimpl_cl_stf::process_cl_stf;
use strata_proofimpl_evm_ee_stf::process_block_transaction_outer;
use zkaleido_native_adapter::{NativeHost, NativeMachine};

use crate::ProofVm;

/// A mock verification key used in native mode when proof verification is not performed.
///
/// This constant provides a placeholder value for scenarios where a verification key is
/// required by a function signature, but actual verification is skipped.
const MOCK_VK: [u32; 8] = [0u32; 8];

/// A native host for [`ProofVm::BtcProving`] prover.
static BTC_BLOCKSPACE_HOST: LazyLock<NativeHost> = std::sync::LazyLock::new(|| NativeHost {
    process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
        process_blockscan_proof(zkvm);
        Ok(())
    })),
});

/// A native host for [`ProofVm::ELProving`] prover.
static EVM_EE_STF_HOST: LazyLock<NativeHost> = std::sync::LazyLock::new(|| NativeHost {
    process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
        process_block_transaction_outer(zkvm);
        Ok(())
    })),
});

/// A native host for [`ProofVm::CLProving`] prover.
static CL_STF_HOST: LazyLock<NativeHost> = std::sync::LazyLock::new(|| NativeHost {
    process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
        process_cl_stf(zkvm, &MOCK_VK, &MOCK_VK);
        Ok(())
    })),
});

/// A native host for [`ProofVm::Checkpoint`] prover.
static CHECKPOINT_HOST: LazyLock<NativeHost> = std::sync::LazyLock::new(|| NativeHost {
    process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
        process_checkpoint_proof_outer(zkvm, &MOCK_VK);
        Ok(())
    })),
});

/// Maps the [`ProofVm`] onto the corresponding Native Host.
pub fn get_host(vm: ProofVm) -> &'static NativeHost {
    match vm {
        ProofVm::BtcProving => &BTC_BLOCKSPACE_HOST,
        ProofVm::ELProving => &EVM_EE_STF_HOST,
        ProofVm::CLProving => &CL_STF_HOST,
        ProofVm::Checkpoint => &CHECKPOINT_HOST,
    }
}
