use std::sync::Arc;

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zkvm_primitives::ZKVMInput;

use crate::task_tracker::TaskTracker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Witness {
    ElBlock(ELBlockWitness),
    ClBlock(CLBlockWitness),
}

impl Witness {
    pub fn get_vm_id(&self) -> u8 {
        match self {
            Witness::ElBlock(witness) => witness.get_vm_id(),
            Witness::ClBlock(witness) => witness.get_vm_id(),
        }
    }
}

impl Default for Witness {
    fn default() -> Self {
        Witness::ElBlock(ELBlockWitness::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ELBlockWitness {
    pub data: Vec<u8>,
}

impl ELBlockWitness {
    pub fn get_vm_id(&self) -> u8 {
        0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CLBlockWitness {
    pub data: Vec<u8>,
}

impl CLBlockWitness {
    pub fn get_vm_id(&self) -> u8 {
        1
    }
}

/// Represents the possible modes of execution for a zkVM program
pub enum ProofGenConfig {
    /// Skips proving.
    Skip,
    /// The simulator runs the rollup verifier logic without even emulating the zkVM
    // Simulate(StateTransitionVerifier<Stf, Da::Verifier, Vm::Guest>),
    /// The executor runs the rollup verification logic in the zkVM, but does not actually
    /// produce a zk proof
    Execute,
    /// The prover runs the rollup verification logic in the zkVM and produces a zk proof
    Prover,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WitnessSubmissionStatus {
    /// The witness has been submitted to the prover.
    SubmittedForProving,
    /// The witness is already present in the prover.
    WitnessExist,
}

/// Represents the status of a DA proof submission.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofSubmissionStatus {
    /// Indicates successful submission of the proof to the DA.
    Success,
    /// Indicates that proof generation is currently in progress.
    ProofGenerationInProgress,
}

/// Represents the current status of proof generation.
#[derive(Debug, Eq, PartialEq)]
pub enum ProofProcessingStatus {
    /// Indicates that proof generation is currently in progress.
    ProvingInProgress,
    /// Indicates that the prover is busy and will not initiate a new proving process.
    Busy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub el_block_num: u64,
    pub witness: Witness,
    pub status: TaskStatus,
}

#[derive(Clone)]
pub struct RpcContext {
    pub task_tracker: Arc<TaskTracker>,
    sequencer_rpc_url: String,
    reth_rpc_url: String,
    el_rpc_client: HttpClient,
}

impl RpcContext {
    pub fn new(
        task_tracker: Arc<TaskTracker>,
        sequencer_rpc_url: String,
        reth_rpc_url: String,
    ) -> Self {
        let el_rpc_client = HttpClientBuilder::default()
            .build(&reth_rpc_url)
            .expect("failed to connect to the el client");

        RpcContext {
            task_tracker,
            sequencer_rpc_url,
            reth_rpc_url,
            el_rpc_client,
        }
    }

    pub fn el_client(&self) -> &HttpClient {
        &self.el_rpc_client
    }
}
