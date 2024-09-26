mod endpoints;
use express_zkvm::{Proof, VerificationKey};
use sp1_sdk::SP1Stdin;

pub fn prove_using_network(
    elf: &[u8],
    input: SP1Stdin,
) -> anyhow::Result<(Proof, VerificationKey)> {
    Ok((Proof::new(Vec::new()), VerificationKey(Vec::new())))
}
