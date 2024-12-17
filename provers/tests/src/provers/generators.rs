use core::panic;
use std::{collections::HashMap, sync::LazyLock};

use btc::BtcBlockProofGenerator;
use checkpoint::CheckpointProofGenerator;
use cl::ClProofGenerator;
use el::ElProofGenerator;
use l1_batch::L1BatchProofGenerator;
use l2_batch::L2BatchProofGenerator;
use strata_native_zkvm_adapter::NativeHost;
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
use strata_zkvm::ZkVmHost;
#[cfg(feature = "risc0")]
use strata_zkvm_hosts::get_risc0_host;
#[cfg(feature = "sp1")]
use strata_zkvm_hosts::get_sp1_host;
use strata_zkvm_hosts::{get_native_host, ProofVm};

/// Test prover generator for the SP1 Host.
#[cfg(feature = "sp1")]
pub static TEST_SP1_GENERATORS: LazyLock<TestProverGenerators<SP1Host>> =
    std::sync::LazyLock::new(|| TestProverGenerators::init(|vm| get_sp1_host(vm).clone()));

/// Test prover generator for the RISC0 Host.
#[cfg(feature = "risc0")]
pub static TEST_RISC0_GENERATORS: LazyLock<TestProverGenerators<Risc0Host>> =
    std::sync::LazyLock::new(|| TestProverGenerators::init(|vm| get_risc0_host(vm).clone()));

/// Test prover generator for the Native Host.
pub static TEST_NATIVE_GENERATORS: LazyLock<TestProverGenerators<NativeHost>> =
    std::sync::LazyLock::new(|| TestProverGenerators::init(|vm| get_native_host(vm).clone()));

use super::{btc, checkpoint, cl, el, l1_batch, l2_batch};

/// A container for the test prover generators for all types, parametrized by the host.
///
/// Corresponds to [`ProofVm`] enum.
#[derive(Clone)]
enum TestGenerator<H: ZkVmHost> {
    BtcBlock(BtcBlockProofGenerator<H>),
    ElBlock(ElProofGenerator<H>),
    ClBlock(ClProofGenerator<H>),
    L1Batch(L1BatchProofGenerator<H>),
    L2Batch(L2BatchProofGenerator<H>),
    Checkpoint(CheckpointProofGenerator<H>),
}

pub struct TestProverGenerators<H: ZkVmHost> {
    generators: HashMap<ProofVm, TestGenerator<H>>,
}

impl<H: ZkVmHost> TestProverGenerators<H> {
    pub fn init<F>(host_provider: F) -> Self
    where
        F: Fn(ProofVm) -> H,
    {
        let mut generators = HashMap::new();

        // TODO: refactor deeper to remove clones.
        // Likely not critical right now due to its being used in tests and perf CI (later).
        let btc_prover = BtcBlockProofGenerator::new(host_provider(ProofVm::BtcProving));
        let l1_batch_prover =
            L1BatchProofGenerator::new(btc_prover.clone(), host_provider(ProofVm::L1Batch));
        let el_prover = ElProofGenerator::new(host_provider(ProofVm::ELProving));
        let cl_prover = ClProofGenerator::new(el_prover.clone(), host_provider(ProofVm::CLProving));
        let l2_batch_prover =
            L2BatchProofGenerator::new(cl_prover.clone(), host_provider(ProofVm::CLAggregation));
        let checkpoint_prover = CheckpointProofGenerator::new(
            l1_batch_prover.clone(),
            l2_batch_prover.clone(),
            host_provider(ProofVm::Checkpoint),
        );

        generators.insert(
            ProofVm::BtcProving,
            TestGenerator::BtcBlock(btc_prover.clone()),
        );
        generators.insert(
            ProofVm::L1Batch,
            TestGenerator::L1Batch(l1_batch_prover.clone()),
        );
        generators.insert(
            ProofVm::ELProving,
            TestGenerator::ElBlock(el_prover.clone()),
        );
        generators.insert(
            ProofVm::CLProving,
            TestGenerator::ClBlock(cl_prover.clone()),
        );
        generators.insert(
            ProofVm::CLAggregation,
            TestGenerator::L2Batch(l2_batch_prover.clone()),
        );
        generators.insert(
            ProofVm::Checkpoint,
            TestGenerator::Checkpoint(checkpoint_prover.clone()),
        );

        Self { generators }
    }

    pub fn btc_blockspace(&self) -> BtcBlockProofGenerator<H> {
        match self.generators.get(&ProofVm::BtcProving).unwrap() {
            TestGenerator::BtcBlock(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }

    pub fn el_block(&self) -> ElProofGenerator<H> {
        match self.generators.get(&ProofVm::ELProving).unwrap() {
            TestGenerator::ElBlock(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }

    pub fn cl_block(&self) -> ClProofGenerator<H> {
        match self.generators.get(&ProofVm::CLProving).unwrap() {
            TestGenerator::ClBlock(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }

    pub fn l1_batch(&self) -> L1BatchProofGenerator<H> {
        match self.generators.get(&ProofVm::L1Batch).unwrap() {
            TestGenerator::L1Batch(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }

    pub fn l2_batch(&self) -> L2BatchProofGenerator<H> {
        match self.generators.get(&ProofVm::CLAggregation).unwrap() {
            TestGenerator::L2Batch(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }

    pub fn checkpoint(&self) -> CheckpointProofGenerator<H> {
        match self.generators.get(&ProofVm::Checkpoint).unwrap() {
            TestGenerator::Checkpoint(value) => value.clone(),
            _ => panic!("unexpected"),
        }
    }
}
