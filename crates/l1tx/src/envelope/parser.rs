use bitcoin::{
    opcodes::all::OP_IF,
    script::{Instruction, Instructions},
    ScriptBuf,
};
use strata_primitives::l1::payload::L1Payload;
use thiserror::Error;
use tracing::debug;

use crate::utils::{next_bytes, next_int, next_op};

pub const ROLLUP_NAME_TAG: &[u8] = &[1];
pub const VERSION_TAG: &[u8] = &[2];
pub const BATCH_DATA_TAG: &[u8] = &[3];

/// Errors that can be generated while parsing envelopes.
#[derive(Debug, Error)]
pub enum EnvelopeParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope(NO OP_IF..OP_ENDIF): ")]
    InvalidEnvelope,
    /// Does not have a valid name tag
    #[error("Invalid/Missing name tag")]
    InvalidNameTag,
    /// Does not have a valid name value
    #[error("Invalid/Missing value")]
    InvalidNameValue,
    // Does not have a valid version tag
    #[error("Invalid/Missing version tag")]
    InvalidVersionTag,
    // Does not have a valid version
    #[error("Invalid/Missing version")]
    InvalidVersion,
    /// Does not have a valid blob tag
    #[error("Invalid/Missing blob tag")]
    InvalidBlobTag,
    /// Does not have a valid blob
    #[error("Invalid/Missing blob tag")]
    InvalidBlob,
    /// Does not have a valid format
    #[error("Invalid Format")]
    InvalidFormat,
}

/// Parse [`L1Payload`]
///
/// # Errors
///
/// This function errors if it cannot parse the [`L1Payload`]
pub fn parse_envelope_data(
    script: &ScriptBuf,
    rollup_name: &str,
) -> Result<L1Payload, EnvelopeParseError> {
    let mut instructions = script.instructions();

    enter_envelope(&mut instructions)?;
    // Parse name
    let (tag, name) = parse_bytes_pair(&mut instructions)?;

    let extracted_rollup_name = match (tag, name) {
        (ROLLUP_NAME_TAG, namebytes) => {
            String::from_utf8(namebytes.to_vec()).map_err(|_| EnvelopeParseError::InvalidNameValue)
        }
        _ => Err(EnvelopeParseError::InvalidNameTag),
    }?;

    if extracted_rollup_name != rollup_name {
        return Err(EnvelopeParseError::InvalidNameTag);
    }

    // Parse version
    let (tag, ver) = parse_bytes_pair(&mut instructions)?;
    let _version = match (tag, ver) {
        (VERSION_TAG, [v]) => Ok(v),
        (VERSION_TAG, _) => Err(EnvelopeParseError::InvalidVersion),
        _ => Err(EnvelopeParseError::InvalidVersionTag),
    }?;

    // Parse bytes
    let tag = next_bytes(&mut instructions).ok_or(EnvelopeParseError::InvalidBlobTag)?;
    let size = next_int(&mut instructions);
    match (tag, size) {
        (BATCH_DATA_TAG, Some(size)) => {
            let batch_data = extract_n_bytes(size, &mut instructions)?;
            Ok(L1Payload::new_checkpoint(batch_data)) // TODO: later this will discern checkpoint
                                                      // and da and any other payload types
        }
        (BATCH_DATA_TAG, None) => Err(EnvelopeParseError::InvalidBlob),
        _ => Err(EnvelopeParseError::InvalidBlobTag),
    }
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

fn parse_bytes_pair<'a>(
    instructions: &mut Instructions<'a>,
) -> Result<(&'a [u8], &'a [u8]), EnvelopeParseError> {
    let tag = next_bytes(instructions).ok_or(EnvelopeParseError::InvalidFormat)?;
    let name = next_bytes(instructions).ok_or(EnvelopeParseError::InvalidFormat)?;
    Ok((tag, name))
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
        Err(EnvelopeParseError::InvalidBlob)
    }
}

#[cfg(test)]
mod tests {

    use strata_btcio::test_utils::generate_envelope_script_test;
    use strata_primitives::l1::payload::L1Payload;

    use super::*;

    #[test]
    fn test_parse_envelope_data() {
        let bytes = vec![0, 1, 2, 3];
        let envelope_data = L1Payload::new_checkpoint(bytes.clone());
        let script = generate_envelope_script_test(envelope_data.clone(), "TestRollup", 1).unwrap();

        // Parse the rollup name
        let result = parse_envelope_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);

        // Try with larger size
        let bytes = vec![1; 2000];
        let envelope_data = L1Payload::new_checkpoint(bytes.clone());
        let script = generate_envelope_script_test(envelope_data.clone(), "TestRollup", 1).unwrap();

        // Parse the rollup name
        let result = parse_envelope_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);
    }
}
