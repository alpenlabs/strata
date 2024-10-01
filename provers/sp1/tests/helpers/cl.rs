use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use express_proofimpl_evm_ee_stf::ELProofInput;
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use express_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};

pub fn get_cl_stf_proof(witness_path: &Path) -> (Proof, VerificationKey) {
    let json_file = fs::read_to_string(witness_path).expect("Failed to read JSON file");
    let el_proof_input: ELProofInput =
        serde_json::from_str(&json_file).expect("Failed to parse JSON");
    let block_num = el_proof_input.parent_header.number + 1;
    let witness_dir = witness_path
        .parent()
        .expect("Witness path has no parent directory");
    let proofs_dir: PathBuf = witness_dir.join("el_proofs");
    if !proofs_dir.exists() {
        fs::create_dir(&proofs_dir)
            .expect("Failed to create el_proofs directory inside witness folder");
    }
    let proof_file = proofs_dir.join(format!("proof_{}.bin", block_num));
    if proof_file.exists() {
        println!("Proof found in cache, returing the cached proof ...");
        return read_proof_from_file(&proof_file);
    }

    println!("Proof not found in cache, generating the proof ...");
    let proof_res = generate_proof(el_proof_input);
    write_proof_to_file(&proof_res, &proof_file);

    proof_res
}

fn read_proof_from_file(proof_file: &Path) -> (Proof, VerificationKey) {
    let mut file = fs::File::open(proof_file).expect("Failed to open existing proof file");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .expect("Failed to read proof file");
    bincode::deserialize(&buffer).expect("Failed to deserialize proof")
}

fn write_proof_to_file(proof_res: &(Proof, VerificationKey), proof_file: &Path) {
    let serialized_proof = bincode::serialize(proof_res).expect("Failed to serialize proof");
    let mut file = fs::File::create(proof_file).expect("Failed to create proof file");
    file.write_all(&serialized_proof)
        .expect("Failed to write proof to file");
}

fn generate_proof(el_proof_input: ELProofInput) -> (Proof, VerificationKey) {
    let prover_ops = ProverOptions {
        enable_compression: true,
        use_mock_prover: false,
        ..Default::default()
    };
    let prover = SP1Host::init(GUEST_EVM_EE_STF_ELF.into(), prover_ops);
    let proof_input = SP1ProofInputBuilder::new()
        .write(&el_proof_input)
        .unwrap()
        .build()
        .unwrap();
    prover.prove(proof_input).expect("Failed to generate proof")
}
