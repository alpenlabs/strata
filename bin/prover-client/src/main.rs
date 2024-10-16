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

    let task_tracker = Arc::new(TaskTracker::new());

    // Create L1 operations
    let btc_ops = BtcOperations::new(btc_client.clone());
    let btc_dispatcher = TaskDispatcher::new(btc_ops, task_tracker.clone());

    // Create EL  operations
    let el_ops = ElOperations::new(el_client.clone());
    let el_dispatcher = TaskDispatcher::new(el_ops, task_tracker.clone());

    let cl_ops = ClOperations::new(cl_client.clone(), Arc::new(el_dispatcher.clone()));
    let cl_dispatcher = TaskDispatcher::new(cl_ops, task_tracker.clone());

    let l1_batch_ops = L1BatchOperations::new(Arc::new(btc_dispatcher.clone()), btc_client.clone());
    let l1_batch_dispatcher = TaskDispatcher::new(l1_batch_ops, task_tracker.clone());

    let l2_batch_ops = L2BatchOperations::new(Arc::new(cl_dispatcher.clone()).clone());
    let l2_batch_dispatcher = TaskDispatcher::new(l2_batch_ops, task_tracker.clone());

    let checkpoint_ops = CheckpointOperations::new(
        cl_client.clone(),
        Arc::new(l1_batch_dispatcher.clone()),
        Arc::new(l2_batch_dispatcher.clone()),
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
    use sp1_sdk::SP1ProofWithPublicValues;
    use strata_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
    use strata_proofimpl_checkpoint::L2BatchProofOutput;
    use strata_proofimpl_evm_ee_stf::ELProofPublicParams;
    use strata_proofimpl_l1_batch::L1BatchProofOutput;
    use strata_sp1_adapter::SP1Verifier;
    use strata_state::{
        batch::{BatchCheckpoint, CheckpointProofOutput},
        l1::{get_btc_params, HeaderVerificationState},
    };
    use strata_zkvm::{Proof, ZKVMVerifier};

    #[test]
    fn test_batch_checkpoint() {
        let batch_checkpoint_raw =
            include_bytes!("../../../functional-tests/sp1_batch_checkpoint.bin");
        let batch_checkpoint: BatchCheckpoint = borsh::from_slice(batch_checkpoint_raw).unwrap();
        // println!("batch_checkpoint {:?}", batch_checkpoint);
        println!("batch_checkpoint pp {:?}", batch_checkpoint.proof_output());

        let ckp_proof_raw = include_bytes!(
            "../../../functional-tests/proofrequest_01jaamw46dfvhrhvzaea8py6nw.cs_proof"
        );
        let proof = Proof::new(ckp_proof_raw.to_vec());
        let ckp_pp: CheckpointProofOutput =
            SP1Verifier::extract_borsh_public_output(&proof).unwrap();

        println!("got the proof pp {:?}", ckp_pp);

        let btc_block_proof_501 = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaammsjqfvhrrcfr6pnbtync.c_proof"
            )
            .to_vec(),
        );
        let btc_block_proof_501_pp: BlockspaceProofOutput =
            SP1Verifier::extract_borsh_public_output(&btc_block_proof_501).unwrap();

        let btc_block_proof_502 = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaammszjeykt0z68qvsfmarv.c_proof"
            )
            .to_vec(),
        );
        let btc_block_proof_502_pp: BlockspaceProofOutput =
            SP1Verifier::extract_borsh_public_output(&btc_block_proof_502).unwrap();

        let btc_block_proof_503 = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaammt27eyksdxg3bw2h940t.c_proof"
            )
            .to_vec(),
        );
        let btc_block_proof_503_pp: BlockspaceProofOutput =
            SP1Verifier::extract_borsh_public_output(&btc_block_proof_503).unwrap();

        let btc_block_proof_504 = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaammsxxeykt8h6t4af7sh3m.c_proof"
            )
            .to_vec(),
        );
        let btc_block_proof_504_pp: BlockspaceProofOutput =
            SP1Verifier::extract_borsh_public_output(&btc_block_proof_504).unwrap();

        let l1_batch_proof = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaamq3n7eykrekn60bjdf98n.c_proof"
            )
            .to_vec(),
        );
        let l1_batch_pp: L1BatchProofOutput =
            SP1Verifier::extract_borsh_public_output(&l1_batch_proof).unwrap();
        println!("{:?}", l1_batch_pp);

        let btc_block_proof_2 = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaammtheeykv7p82mpxnxrtp.c_proof"
            )
            .to_vec(),
        );
        let btc_block_proof_2_pp: ELProofPublicParams =
            SP1Verifier::extract_public_output(&btc_block_proof_2).unwrap();
        println!("EL");

        let cl_block_proof = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaamt3vjfvhsgmfvapntqw70.c_proof"
            )
            .to_vec(),
        );
        let cl_block_proof_pp: L2BatchProofOutput =
            SP1Verifier::extract_borsh_public_output(&cl_block_proof).unwrap();

        let l2_batch_proof = Proof::new(
            include_bytes!(
                "../../../functional-tests/proofrequest_01jaamqk0hemqs6s3jx46avt3h.c_proof"
            )
            .to_vec(),
        );
        let l2_batch_proof_pp: L2BatchProofOutput =
            SP1Verifier::extract_borsh_public_output(&l2_batch_proof).unwrap();

        let l1_batch_pp: SP1ProofWithPublicValues =
            bincode::deserialize(l1_batch_proof.as_bytes()).unwrap();
        let mut header_vs: HeaderVerificationState =
            borsh::from_slice(&l1_batch_pp.stdin.buffer[1]).unwrap();
        let params = get_btc_params();

        println!("{:?}", header_vs.compute_initial_snapshot());
        println!("{:?}", header_vs.compute_final_snapshot());
        header_vs.check_and_update_continuity(
            &bitcoin::consensus::deserialize(&btc_block_proof_501_pp.header_raw).unwrap(),
            &params,
        );
        println!("{:?}", header_vs.compute_initial_snapshot());
        println!("{:?}", header_vs.compute_final_snapshot());

        header_vs.check_and_update_continuity(
            &bitcoin::consensus::deserialize(&btc_block_proof_502_pp.header_raw).unwrap(),
            &params,
        );
        println!("{:?}", header_vs.compute_initial_snapshot());
        println!("{:?}", header_vs.compute_final_snapshot());

        header_vs.check_and_update_continuity(
            &bitcoin::consensus::deserialize(&btc_block_proof_503_pp.header_raw).unwrap(),
            &params,
        );
        println!("{:?}", header_vs.compute_initial_snapshot());
        println!("{:?}", header_vs.compute_final_snapshot());

        header_vs.check_and_update_continuity(
            &bitcoin::consensus::deserialize(&btc_block_proof_504_pp.header_raw).unwrap(),
            &params,
        );
        println!("{:?}", header_vs.compute_initial_snapshot());
        println!("{:?}", header_vs.compute_final_snapshot());
    }
}
