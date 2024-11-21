use anyhow::Result;
use bitcoin::params::MAINNET;
use strata_native_zkvm_adapter::{NativeHost, NativeMachine};
use strata_proofimpl_l1_batch::{process_l1_batch_proof, L1BatchProofInput, L1BatchProver};
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_test_utils::bitcoin::get_btc_chain;
use strata_zkvm::{ProofWithInfo, ZkVmHost, ZkVmProver};

use crate::{btc::BtcBlockProofGenerator, proof_generator::ProofGenerator};

pub struct L1BatchProofGenerator<H: ZkVmHost> {
    btc_proof_generator: BtcBlockProofGenerator<H>,
    host: H,
}

impl<H: ZkVmHost> L1BatchProofGenerator<H> {
    pub fn new(btc_proof_generator: BtcBlockProofGenerator<H>, host: H) -> Self {
        Self {
            btc_proof_generator,
            host,
        }
    }
}

impl<H: ZkVmHost> ProofGenerator<(u32, u32), L1BatchProver> for L1BatchProofGenerator<H> {
    fn get_input(&self, heights: &(u32, u32)) -> Result<L1BatchProofInput> {
        let (start_height, end_height) = *heights;

        let btc_chain = get_btc_chain();

        let state = btc_chain.get_verification_state(start_height, &MAINNET.clone().into());

        let mut batch = vec![];
        for height in start_height..=end_height {
            let block = btc_chain.get_block(height);
            let btc_proof = self.btc_proof_generator.get_proof(block)?;
            batch.push(btc_proof);
        }

        let input = L1BatchProofInput {
            state,
            batch,
            blockspace_vk: self.btc_proof_generator.get_host().get_verification_key(),
        };
        // dbg!(&input.blockspace_vk);
        Ok(input)
    }

    fn gen_proof(&self, heights: &(u32, u32)) -> Result<ProofWithInfo> {
        let input = self.get_input(heights)?;
        let host = self.get_host();
        L1BatchProver::prove(&input, &host)
    }

    fn get_proof_id(&self, heights: &(u32, u32)) -> String {
        let (start_height, end_height) = *heights;
        format!("l1_batch_{}_{}", start_height, end_height)
    }

    fn get_host(&self) -> impl ZkVmHost {
        self.host.clone()
    }
}

pub fn get_native_host() -> NativeHost {
    use std::sync::Arc;

    NativeHost {
        process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
            process_l1_batch_proof(zkvm, &[0u32; 8]);
            Ok(())
        })),
    }
}

#[cfg(feature = "risc0")]
pub fn get_risc0_host() -> Risc0Host {
    use strata_risc0_guest_builder::GUEST_RISC0_L1_BATCH_ELF;

    Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF)
}

#[cfg(feature = "sp1")]
pub fn get_sp1_host() -> SP1Host {
    use strata_sp1_guest_builder::{GUEST_L1_BATCH_ELF, GUEST_L1_BATCH_PK, GUEST_L1_BATCH_VK};

    SP1Host::new_from_bytes(&GUEST_L1_BATCH_ELF, &GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK)
}

#[cfg(test)]
mod test {
    use strata_test_utils::l2::gen_params;
    use strata_zkvm::ZkVmHost;

    use super::*;
    use crate::btc;

    fn test_proof<H: ZkVmHost>(l1_batch_host: H, btc_blockspace_host: H) {
        let params = gen_params();
        let rollup_params = params.rollup();
        let l1_start_height = (rollup_params.genesis_l1_height + 1) as u32;
        let l1_end_height = l1_start_height + 1;

        let btc_proof_generator = BtcBlockProofGenerator::new(btc_blockspace_host);
        let l1_batch_proof_generator =
            L1BatchProofGenerator::new(btc_proof_generator, l1_batch_host);
        let _ = l1_batch_proof_generator
            .get_proof(&(l1_start_height, l1_end_height))
            .unwrap();
    }

    #[test]
    fn test_native() {
        test_proof(get_native_host(), btc::get_native_host());
    }

    #[test]
    #[cfg(feature = "risc0")]
    fn test_risc0() {
        test_proof(get_risc0_host(), btc::get_risc0_host());
    }

    #[test]
    #[cfg(feature = "sp1")]
    fn test_sp1() {
        test_proof(get_sp1_host(), btc::get_sp1_host());
    }
}
