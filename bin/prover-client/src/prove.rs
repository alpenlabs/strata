use strata_proofimpl_evm_ee_stf::ELProofInput;
use strata_zkvm::{ProofType, ZkVmHost, ZkVmInputBuilder};

use crate::{
    primitives::prover_input::{ProofWithVkey, ZkVmInput},
    proving_ops::btc_ops::get_pm_rollup_params,
};

pub fn make_proof<Vm>(zkvm_input: ZkVmInput, vm: Vm) -> Result<ProofWithVkey, anyhow::Error>
where
    Vm: ZkVmHost + 'static,
    for<'a> Vm::Input<'a>: ZkVmInputBuilder<'a>,
{
    let (zkvm_input, proof_type) = match zkvm_input {
        ZkVmInput::ElBlock(el_input) => {
            let el_input: ELProofInput = bincode::deserialize(&el_input.data)?;
            (
                Vm::Input::new().write_serde(&el_input)?.build()?,
                ProofType::Compressed,
            )
        }

        ZkVmInput::BtcBlock(block, rollup_params) => (
            Vm::Input::new()
                .write_serde(&rollup_params)?
                .write_buf(&bitcoin::consensus::serialize(&block))?
                .build()?,
            ProofType::Compressed,
        ),

        ZkVmInput::L1Batch(l1_batch_input) => {
            let mut input_builder = Vm::Input::new();
            input_builder.write_borsh(&l1_batch_input.header_verification_state)?;
            input_builder.write_serde(&l1_batch_input.btc_task_ids.len())?;
            // Write each proof input
            for proof_input in l1_batch_input.get_proofs() {
                input_builder.write_proof(proof_input)?;
            }

            (input_builder.build()?, ProofType::Compressed)
        }

        ZkVmInput::ClBlock(cl_proof_input) => (
            Vm::Input::new()
                .write_serde(&get_pm_rollup_params())?
                .write_buf(&cl_proof_input.cl_raw_witness)?
                .write_proof(
                    cl_proof_input
                        .el_proof
                        .expect("CL Proving was sent without EL proof"),
                )?
                .build()?,
            ProofType::Compressed,
        ),

        ZkVmInput::L2Batch(l2_batch_input) => {
            let mut input_builder = Vm::Input::new();

            // Write the number of task IDs
            let task_count = l2_batch_input.cl_task_ids.len();
            input_builder.write_serde(&task_count)?;

            // Write each proof input
            for proof_input in l2_batch_input.get_proofs() {
                input_builder.write_proof(proof_input)?;
            }

            (input_builder.build()?, ProofType::Compressed)
        }

        ZkVmInput::Checkpoint(checkpoint_input) => {
            let l1_batch_proof = checkpoint_input
                .l1_batch_proof
                .ok_or_else(|| anyhow::anyhow!("L1 Batch Proof Not Ready"))?;

            let l2_batch_proof = checkpoint_input
                .l2_batch_proof
                .ok_or_else(|| anyhow::anyhow!("L2 Batch Proof Not Ready"))?;

            let mut input_builder = Vm::Input::new();
            input_builder.write_serde(&get_pm_rollup_params())?;
            input_builder.write_proof(l1_batch_proof)?;
            input_builder.write_proof(l2_batch_proof)?;

            (input_builder.build()?, ProofType::Groth16)
        }
    };

    let (proof, _) = vm.prove(zkvm_input, proof_type)?;
    let agg_input = ProofWithVkey::new(proof, vm.get_verification_key());
    Ok(agg_input)
}
