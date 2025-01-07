use core::panic;
use std::collections::HashMap;

use btc::BtcBlockProofGenerator;
use checkpoint::CheckpointProofGenerator;
use cl::ClProofGenerator;
use el::ElProofGenerator;
use l1_batch::L1BatchProofGenerator;
use l2_batch::L2BatchProofGenerator;
use strata_zkvm::ZkVmHost;
use strata_zkvm_hosts::ProofVm;

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
        // Likely not critical right now due to its being used in tests and perf CI.
        let btc_prover = BtcBlockProofGenerator::new(host_provider(ProofVm::BtcProving));
        let l1_batch_prover = L1BatchProofGenerator::new(host_provider(ProofVm::L1Batch));
        let el_prover = ElProofGenerator::new(host_provider(ProofVm::ELProving));
        let cl_prover = ClProofGenerator::new(el_prover.clone(), host_provider(ProofVm::CLProving));
        let l2_batch_prover =
            L2BatchProofGenerator::new(cl_prover.clone(), host_provider(ProofVm::CLAggregation));
        let checkpoint_prover = CheckpointProofGenerator::new(
            l1_batch_prover.clone(),
            l2_batch_prover.clone(),
            host_provider(ProofVm::Checkpoint),
        );

        generators.insert(ProofVm::BtcProving, TestGenerator::BtcBlock(btc_prover));
        generators.insert(ProofVm::L1Batch, TestGenerator::L1Batch(l1_batch_prover));
        generators.insert(ProofVm::ELProving, TestGenerator::ElBlock(el_prover));
        generators.insert(ProofVm::CLProving, TestGenerator::ClBlock(cl_prover));
        generators.insert(
            ProofVm::CLAggregation,
            TestGenerator::L2Batch(l2_batch_prover),
        );
        generators.insert(
            ProofVm::Checkpoint,
            TestGenerator::Checkpoint(checkpoint_prover),
        );

        Self { generators }
    }

    pub fn btc_blockspace(&self) -> &BtcBlockProofGenerator<H> {
        match self.generators.get(&ProofVm::BtcProving).unwrap() {
            TestGenerator::BtcBlock(value) => value,
            _ => panic!("unexpected"),
        }
    }

    pub fn el_block(&self) -> &ElProofGenerator<H> {
        match self.generators.get(&ProofVm::ELProving).unwrap() {
            TestGenerator::ElBlock(value) => value,
            _ => panic!("unexpected"),
        }
    }

    pub fn cl_block(&self) -> &ClProofGenerator<H> {
        match self.generators.get(&ProofVm::CLProving).unwrap() {
            TestGenerator::ClBlock(value) => value,
            _ => panic!("unexpected"),
        }
    }

    pub fn l1_batch(&self) -> &L1BatchProofGenerator<H> {
        match self.generators.get(&ProofVm::L1Batch).unwrap() {
            TestGenerator::L1Batch(value) => value,
            _ => panic!("unexpected"),
        }
    }

    pub fn l2_batch(&self) -> &L2BatchProofGenerator<H> {
        match self.generators.get(&ProofVm::CLAggregation).unwrap() {
            TestGenerator::L2Batch(value) => value,
            _ => panic!("unexpected"),
        }
    }

    pub fn checkpoint(&self) -> &CheckpointProofGenerator<H> {
        match self.generators.get(&ProofVm::Checkpoint).unwrap() {
            TestGenerator::Checkpoint(value) => value,
            _ => panic!("unexpected"),
        }
    }
}
