use std::str::from_utf8;

use bitcoin::{
    opcodes::all::OP_IF,
    script::{Instruction, Instructions},
    ScriptBuf,
};
use strata_primitives::{
    l1::payload::{L1Payload, L1PayloadType},
    params::RollupParams,
};
use thiserror::Error;
use tracing::{debug, warn};

use crate::utils::{next_bytes, next_op, next_u32};

pub const ROLLUP_NAME_TAG: &[u8] = &[1];
pub const VERSION_TAG: &[u8] = &[2];
pub const BATCH_DATA_TAG: &[u8] = &[3];

/// Errors that can be generated while parsing envelopes.
#[derive(Debug, Error)]
pub enum EnvelopeParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope(NO OP_IF..OP_ENDIF): ")]
    InvalidEnvelope,
    /// Does not have a valid tag
    #[error("Invalid/Missing tag")]
    InvalidTag,
    // Does not have a valid version
    #[error("Invalid/Missing version")]
    InvalidVersion,
    // Does not have a valid size
    #[error("Invalid/Missing size")]
    InvalidSize,
    /// Does not have a valid format
    #[error("Invalid Format")]
    InvalidFormat,
    /// Does not have a payload data of expected size
    #[error("Invalid Payload")]
    InvalidPayload,
}

/// Parse [`L1Payload`]
///
/// # Errors
///
/// This function errors if it cannot parse the [`L1Payload`]
pub fn parse_envelope_data(
    script: &ScriptBuf,
    params: &RollupParams,
) -> Result<L1Payload, EnvelopeParseError> {
    let mut instructions = script.instructions();

    enter_envelope(&mut instructions)?;

    // Parse tag
    let tag = next_bytes(&mut instructions)
        .and_then(|bytes| parse_payload_type(bytes, params))
        .ok_or(EnvelopeParseError::InvalidTag)?;

    // Parse version
    let _version = next_bytes(&mut instructions)
        .and_then(validate_version)
        .ok_or(EnvelopeParseError::InvalidVersion)?;

    // Parse size
    let size = next_u32(&mut instructions).ok_or(EnvelopeParseError::InvalidSize)?;
    // Parse payload
    let payload = extract_n_bytes(size, &mut instructions)?;
    Ok(L1Payload::new(payload, tag))
}

fn parse_payload_type(bytes: &[u8], params: &RollupParams) -> Option<L1PayloadType> {
    let str = from_utf8(bytes).ok()?;
    if params.checkpoint_tag == str {
        Some(L1PayloadType::Checkpoint)
    } else if params.da_tag == str {
        Some(L1PayloadType::Da)
    } else {
        None
    }
}

fn validate_version(bytes: &[u8]) -> Option<u8> {
    if bytes.len() != 1 {
        warn!("Invalid version bytes length, should be 1");
        return None;
    }
    let version = bytes[0];
    // TODO: add version validation logic, i.e which particular versions are supported
    Some(version)
}

/// Check for consecutive `OP_FALSE` and `OP_IF` that marks the beginning of an envelope
fn enter_envelope(instructions: &mut Instructions) -> Result<(), EnvelopeParseError> {
    // loop until OP_FALSE is found
    loop {
        let next = instructions.next();
        match next {
            None => {
                return Err(EnvelopeParseError::InvalidEnvelope);
            }
            // OP_FALSE is basically empty PushBytes
            Some(Ok(Instruction::PushBytes(bytes))) => {
                if bytes.as_bytes().is_empty() {
                    break;
                }
            }
            _ => {
                // Just carry on until OP_FALSE is found
            }
        }
    }

    // Check if next opcode is OP_IF
    let op_if = next_op(instructions);
    if op_if != Some(OP_IF) {
        return Err(EnvelopeParseError::InvalidEnvelope);
    }
    Ok(())
}

/// Extract bytes of `size` from the remaining instructions
fn extract_n_bytes(
    size: u32,
    instructions: &mut Instructions,
) -> Result<Vec<u8>, EnvelopeParseError> {
    debug!("Extracting {} bytes from instructions", size);
    let mut data = vec![];
    let mut curr_size: u32 = 0;
    while let Some(bytes) = next_bytes(instructions) {
        data.extend_from_slice(bytes);
        curr_size += bytes.len() as u32;
    }
    if curr_size == size {
        Ok(data)
    } else {
        debug!("Extracting {} bytes from instructions", size);
        Err(EnvelopeParseError::InvalidPayload)
    }
}

#[cfg(test)]
mod tests {

    use strata_btcio::test_utils::generate_envelope_script_test;
    use strata_primitives::l1::payload::L1Payload;
    use strata_test_utils::l2::gen_params;

    use super::*;

    #[test]
    fn test_parse_envelope_data() {
        let bytes = vec![0, 1, 2, 3];
        let params = gen_params();
        let envelope_data = L1Payload::new_checkpoint(bytes.clone());
        let script =
            generate_envelope_script_test(envelope_data.clone(), params.clone().into(), 1).unwrap();

        let result = parse_envelope_data(&script, params.rollup()).unwrap();

        assert_eq!(result, envelope_data);

        // Try with larger size
        let bytes = vec![1; 2000];
        let envelope_data = L1Payload::new_checkpoint(bytes.clone());
        let script =
            generate_envelope_script_test(envelope_data.clone(), params.clone().into(), 1).unwrap();

        // Parse the rollup name
        let result = parse_envelope_data(&script, params.rollup()).unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);
    }
}
