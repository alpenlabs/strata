use core::panic;
use std::collections::HashMap;

use btc::BtcBlockProofGenerator;
use checkpoint::CheckpointProofGenerator;
use cl::ClProofGenerator;
use el::ElProofGenerator;
use strata_zkvm_hosts::ProofVm;
use zkaleido::ZkVmHost;

use super::{btc, checkpoint, cl, el};

/// A container for the test prover generators for all types, parametrized by the host.
///
/// Corresponds to [`ProofVm`] enum.
#[derive(Clone)]
enum TestGenerator<H: ZkVmHost> {
    BtcBlock(BtcBlockProofGenerator<H>),
    ElBlock(ElProofGenerator<H>),
    ClBlock(ClProofGenerator<H>),
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
        let el_prover = ElProofGenerator::new(host_provider(ProofVm::ELProving));
        let cl_prover = ClProofGenerator::new(
            btc_prover.clone(),
            el_prover.clone(),
            host_provider(ProofVm::CLProving),
        );
        let checkpoint_prover =
            CheckpointProofGenerator::new(cl_prover.clone(), host_provider(ProofVm::Checkpoint));

        generators.insert(ProofVm::BtcProving, TestGenerator::BtcBlock(btc_prover));
        generators.insert(ProofVm::ELProving, TestGenerator::ElBlock(el_prover));
        generators.insert(ProofVm::CLProving, TestGenerator::ClBlock(cl_prover));
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

    pub fn checkpoint(&self) -> &CheckpointProofGenerator<H> {
        match self.generators.get(&ProofVm::Checkpoint).unwrap() {
            TestGenerator::Checkpoint(value) => value,
            _ => panic!("unexpected"),
        }
    }
}
