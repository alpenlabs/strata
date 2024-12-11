use std::iter::Peekable;

use bitcoin::{
    opcodes::all::OP_IF,
    script::{Instruction, Instructions},
    ScriptBuf,
};
use strata_state::tx::{EnvelopePayload, PayloadTypeTag};
use thiserror::Error;

use super::utils::{next_bytes, next_int, next_op};

#[derive(Debug, Error)]
pub enum EnvelopeParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope(NO OP_IF..OP_ENDIF): ")]
    InvalidEnvelope,
    /// Does not have a valid name tag
    #[error("Invalid/Missing Batch tag")]
    UnknownTag,
    /// Does not have a valid name value
    #[error("Invalid/Missing value")]
    InvalidNameValue,
    // Does not have a valid version tag
    #[error("Invalid/Missing version tag")]
    InvalidVersionTag,
    // Does not have a valid version
    #[error("Invalid/Missing version")]
    InvalidVersion,
    /// Does not have a valid format
    #[error("Invalid Format")]
    InvalidFormat,
}

pub fn parse_envelope_data(
    script: &ScriptBuf,
    rollup_name: &str,
) -> Result<Vec<EnvelopePayload>, EnvelopeParseError> {
    let mut instructions = script.instructions().peekable();
    let mut index = 0;
    let mut blobs = Vec::new();
    while enter_envelope(&mut instructions).is_ok() {
        blobs.push(parse_envelope(index, &mut instructions, rollup_name)?);
        index += 1;
    }

    Ok(blobs)
}

/// To Parse [`EnvelopePayload`]
///
/// # Errors
///
/// This function errors if it cannot extract the [`EnvelopePayload`] from the bitcoin instructions.
pub fn parse_envelope(
    index: u32,
    instructions: &mut Peekable<Instructions<'_>>,
    rollup_name: &str,
) -> Result<EnvelopePayload, EnvelopeParseError> {
    // Parse name
    if index == 0 {
        let name = next_bytes(instructions).ok_or(EnvelopeParseError::InvalidFormat)?;
        let extracted_rollup_name =
            String::from_utf8(name.to_vec()).map_err(|_| EnvelopeParseError::InvalidNameValue)?;

        if extracted_rollup_name != rollup_name {
            return Err(EnvelopeParseError::InvalidNameValue);
        }
    }
    let tag = next_int(instructions).ok_or(EnvelopeParseError::InvalidFormat)?;

    let batch_data = extract_bytes(instructions)?;
    // DA for now but this should be based on the TAG on the script itself
    Ok(EnvelopePayload::new(
        PayloadTypeTag::try_from(tag as u8).map_err(|_| EnvelopeParseError::UnknownTag)?,
        batch_data,
    ))
}

/// Check for consecutive `OP_FALSE` and `OP_IF` that marks the beginning of an envelope
fn enter_envelope(instructions: &mut Peekable<Instructions<'_>>) -> Result<(), EnvelopeParseError> {
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
fn extract_bytes(
    instructions: &mut Peekable<Instructions<'_>>,
) -> Result<Vec<u8>, EnvelopeParseError> {
    let mut data = vec![];
    while let Some(bytes) = next_bytes(instructions) {
        data.extend_from_slice(bytes);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {

    use strata_btcio::writer::builder::generate_envelope_script;
    use strata_state::tx::PayloadTypeTag;

    use super::*;

    #[test]
    fn test_parse_envelope_data() {
        let bytes = vec![0, 1, 2, 3];
        let envelope_data = vec![
            EnvelopePayload::new(PayloadTypeTag::DA, bytes.clone()),
            EnvelopePayload::new(PayloadTypeTag::Checkpoint, bytes.clone()),
        ];
        let script = generate_envelope_script(&envelope_data, "TestRollup").unwrap();

        // Parse the rollup name
        let result = parse_envelope_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);
        // Try with larger size
        let bytes = vec![1; 2000];
        let envelope_data = vec![EnvelopePayload::new(PayloadTypeTag::DA, bytes.clone())];
        let script = generate_envelope_script(&envelope_data, "TestRollup").unwrap();

        // Parse the rollup name
        let result = parse_envelope_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);
    }
}
