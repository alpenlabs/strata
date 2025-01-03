use std::array::TryFromSliceError;

use revm::primitives::{PrecompileError, PrecompileOutput, PrecompileResult};
use revm_primitives::Bytes;
use strata_crypto::verify_schnorr_sig;
use strata_primitives::buf::{Buf32, Buf64};
use thiserror::Error;

/// 32 Bytes: Public key
/// 32 Bytes: Message hash
/// 64 Bytes: Schnorr Signature
struct SchnorrInput {
    public_key: Buf32,
    message_hash: Buf32,
    signature: Buf64,
}

#[derive(Debug, Error)]
enum SchnorrError {
    #[error("Input not exactly 128 bytes")]
    InvalidInput,

    #[error("Slice conversion failed: {0}")]
    FixedBytes(#[from] TryFromSliceError),
}

fn parse_schnorr_input(input: &Bytes) -> Result<SchnorrInput, SchnorrError> {
    // validate if the length is 128 bytes or not
    if input.len() != 128 {
        return Err(SchnorrError::InvalidInput);
    }

    Ok(SchnorrInput {
        public_key: Buf32::new(input[0..32].try_into().map_err(SchnorrError::FixedBytes)?),
        message_hash: Buf32::new(input[32..64].try_into().map_err(SchnorrError::FixedBytes)?),
        signature: Buf64::new(
            input[64..128]
                .try_into()
                .map_err(SchnorrError::FixedBytes)?,
        ),
    })
}

pub fn schnorr_precompile(input: &Bytes, _gas_limit: u64) -> PrecompileResult {
    let schnorr_input =
        parse_schnorr_input(input).map_err(|err| PrecompileError::other(err.to_string()))?;

    let verification_byte = {
        match verify_schnorr_sig(
            &schnorr_input.signature,
            &schnorr_input.message_hash,
            &schnorr_input.public_key,
        ) {
            true => Bytes::from([0x01]),
            false => Bytes::from([0x00]),
        }
    };

    // currently we can use [ecrecover hack](https://hackmd.io/@nZ-twauPRISEa6G9zg3XRw/SyjJzSLt9)
    // which costs around ~3000 gas.
    // setting it as 0, as this requires further discussion
    let gas_cost = 0;

    Ok(PrecompileOutput::new(gas_cost, verification_byte))
}
