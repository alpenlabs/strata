//! Prover client.

use std::sync::Arc;

use args::Args;
use ckp_runner::start_checkpoints_task;
use dispatcher::TaskDispatcher;
use jsonrpsee::http_client::HttpClientBuilder;
use manager::ProverManager;
use proving_ops::{
    btc_ops::BtcOperations, checkpoint_ops::CheckpointOperations, cl_ops::ClOperations,
    el_ops::ElOperations, l1_batch_ops::L1BatchOperations, l2_batch_ops::L2BatchOperations,
};
use rpc_server::{ProverClientRpc, RpcContext};
use strata_btcio::rpc::BitcoinClient;
use strata_common::logging;
use strata_sp1_adapter::SP1Host;
use strata_zkvm::ProverOptions;
use task::TaskTracker;
use tracing::{debug, info};

mod args;
mod ckp_runner;
mod config;
mod db;
mod dispatcher;
mod errors;
mod manager;
mod primitives;
mod prover;
mod proving_ops;
mod rpc_server;
mod task;

#[tokio::main]
async fn main() {
    logging::init();
    info!("Running strata prover client in dev mode");

    let args: Args = argh::from_env();
    debug!("Running prover client with args {:?}", args);

    let el_client = HttpClientBuilder::default()
        .build(args.get_reth_rpc_url())
        .expect("failed to connect to the el client");

    let cl_client = HttpClientBuilder::default()
        .build(args.get_sequencer_rpc_url())
        .expect("failed to connect to the el client");

    let btc_client = Arc::new(
        BitcoinClient::new(
            args.get_btc_rpc_url(),
            args.bitcoind_user.clone(),
            args.bitcoind_password.clone(),
        )
        .expect("failed to connect to the btc client"),
    );

    let rollup_params = Arc::new(args.resolve_and_validate_rollup_params().unwrap());
    let task_tracker = Arc::new(TaskTracker::new());

    // Create L1 operations
    let btc_ops = BtcOperations::new(btc_client.clone(), rollup_params.clone());
    let btc_dispatcher = TaskDispatcher::new(btc_ops, task_tracker.clone());

    // Create EL  operations
    let el_ops = ElOperations::new(el_client.clone());
    let el_dispatcher = TaskDispatcher::new(el_ops, task_tracker.clone());

    let cl_ops = ClOperations::new(
        cl_client.clone(),
        Arc::new(el_dispatcher.clone()),
        rollup_params.clone(),
    );
    let cl_dispatcher = TaskDispatcher::new(cl_ops, task_tracker.clone());

    let l1_batch_ops = L1BatchOperations::new(
        Arc::new(btc_dispatcher.clone()),
        btc_client.clone(),
        rollup_params.clone(),
    );
    let l1_batch_dispatcher = TaskDispatcher::new(l1_batch_ops, task_tracker.clone());

    let l2_batch_ops = L2BatchOperations::new(Arc::new(cl_dispatcher.clone()).clone());
    let l2_batch_dispatcher = TaskDispatcher::new(l2_batch_ops, task_tracker.clone());

    let checkpoint_ops = CheckpointOperations::new(
        cl_client.clone(),
        Arc::new(l1_batch_dispatcher.clone()),
        Arc::new(l2_batch_dispatcher.clone()),
        rollup_params.clone(),
    );

    let checkpoint_dispatcher = TaskDispatcher::new(checkpoint_ops, task_tracker.clone());

    let rpc_context = RpcContext::new(
        btc_dispatcher.clone(),
        el_dispatcher.clone(),
        cl_dispatcher.clone(),
        l1_batch_dispatcher.clone(),
        l2_batch_dispatcher.clone(),
        checkpoint_dispatcher.clone(),
    );

    let prover_options = ProverOptions {
        use_mock_prover: false,
        enable_compression: true,
        ..Default::default()
    };
    let prover_manager: ProverManager<SP1Host> =
        ProverManager::new(task_tracker.clone(), prover_options);

    // run prover manager in background
    tokio::spawn(async move { prover_manager.run().await });

    // run checkpoint runner
    tokio::spawn(async move {
        start_checkpoints_task(
            cl_client.clone(),
            checkpoint_dispatcher.clone(),
            task_tracker.clone(),
        )
        .await
    });

    // Run prover manager in dev mode or runner mode
    if args.enable_dev_rpcs {
        // Run the rpc server on dev mode only
        let rpc_url = args.get_dev_rpc_url();
        run_rpc_server(rpc_context, rpc_url, args.enable_dev_rpcs)
            .await
            .expect("prover client rpc")
    }
}

async fn run_rpc_server(
    rpc_context: RpcContext,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()> {
    let rpc_impl = ProverClientRpc::new(rpc_context);
    rpc_server::start(&rpc_impl, rpc_url, enable_dev_rpc).await?;
    anyhow::Ok(())
}

#[cfg(test)]
mod test {
    use sp1_sdk::SP1Prover;
    use strata_primitives::{params::RollupParams, vk::RollupVerifyingKey};
    use strata_proofimpl_checkpoint::{process_checkpoint_proof, L2BatchProofOutput};
    use strata_proofimpl_l1_batch::L1BatchProofOutput;
    use strata_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
    use strata_sp1_guest_builder::GUEST_CHECKPOINT_ELF;
    use strata_state::batch::CheckpointProofOutput;
    use strata_zkvm::{AggregationInput, ProverOptions, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier};

    use crate::config::CHECKPOINT_POLL_INTERVAL;

    #[test]
    fn make_one() {
        // CKP 1 here
        let input = include_bytes!(
            "../../../functional-tests/witness_5981db93-01a9-4fb8-946c-5caa4cd55d97.bin"
        );

        let (rollup_params, l1, l2): (RollupParams, AggregationInput, AggregationInput) =
            bincode::deserialize(input).unwrap();

        let l1_batch_proof = l1.proof();
        let l1_batch_pp: L1BatchProofOutput =
            SP1Verifier::extract_borsh_public_output(l1_batch_proof).unwrap();

        let l2_batch_proof = l2.proof();
        let l2_batch_pp: L2BatchProofOutput =
            SP1Verifier::extract_borsh_public_output(l2_batch_proof).unwrap();

        let (output, prev_checkpoint) =
            process_checkpoint_proof(&l1_batch_pp, &l2_batch_pp, &rollup_params);

        if let Some(prev_checkpoint) = prev_checkpoint {
            let (checkpoint, proof) = prev_checkpoint;
            let rollup_vk = match rollup_params.rollup_vk() {
                RollupVerifyingKey::SP1VerifyingKey(sp1_vk) => sp1_vk,
                _ => panic!("Need SP1VerifyingKey"),
            };
            SP1Verifier::verify_groth16_raw(
                &proof,
                rollup_vk.as_bytes(),
                &borsh::to_vec(&checkpoint).unwrap(),
                // &bincode::serialize(&borsh::to_vec(&checkpoint).unwrap()).unwrap(),
            )
            .unwrap();
        }

        // println!(
        //     "Is prev proof present {:?}",
        //     l1_batch_pp.prev_checkpoint.clone().unwrap().batch_info()
        // );
        // println!(
        //     "Is prev proof present {:?}",
        //     l1_batch_pp.prev_checkpoint.unwrap().bootstrap_state()
        // );

        // // CKP 0 here
        // let input = include_bytes!(
        //     "../../../functional-tests/witness_b3920c9a-a0ec-42ae-8e08-c432ccdd6e77.bin" /*
        // "../../../functional-tests/witness_5981db93-01a9-4fb8-946c-5caa4cd55d97.bin" */
        // );

        // let pp = include_bytes!("../proof_public_param.bin");
        // let pp: CheckpointProofOutput = borsh::from_slice(pp).unwrap();
        // println!("Obt pp: {:?}", pp);

        // let pp: CheckpointProofOutput =
        //     SP1Verifier::extract_borsh_public_output(proof.0.proof()).unwrap();

        // let (rollup_params, l1, l2): (RollupParams, AggregationInput, AggregationInput) =
        //     bincode::deserialize(input).unwrap();

        // let vm = SP1Host::init(
        //     GUEST_CHECKPOINT_ELF.into(),
        //     ProverOptions {
        //         use_cached_keys: true,
        //         enable_compression: true,
        //         stark_to_snark_conversion: true,
        //         use_mock_prover: false,
        //     },
        // );

        // let mut input_builder = SP1ProofInputBuilder::new();
        // input_builder.write(&rollup_params).unwrap();
        // input_builder.write_proof(l1).unwrap();
        // input_builder.write_proof(l2).unwrap();
        // let input = input_builder.build().unwrap();

        // let proof = vm.prove(input).unwrap();
        // let pp: CheckpointProofOutput =
        //     SP1Verifier::extract_borsh_public_output(proof.0.proof()).unwrap();
        // println!("Obt pp: {:?}", pp);
        // let pp_ser = borsh::to_vec(&pp).unwrap();

        // use std::{fs::File, io::Write};
        // let filename = "proof_public_param.bin".to_string();
        // let mut file = File::create(filename).unwrap();
        // file.write_all(&pp_ser).unwrap();
        println!("******************");
    }
}
