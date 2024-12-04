use std::iter::Peekable;

use bitcoin::{
    opcodes::all::OP_IF,
    script::{Instruction, Instructions},
    ScriptBuf,
};
use strata_state::tx::{BlobType, InscriptionBlob};
use thiserror::Error;
use tracing::debug;

use super::utils::{next_bytes, next_int, next_op};

pub const ROLLUP_NAME_TAG: &[u8] = &[1];
pub const VERSION_TAG: &[u8] = &[2];
pub const BATCH_DATA_TAG: &[u8] = &[3];

#[derive(Debug, Error)]
pub enum InscriptionParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope(NO OP_IF..OP_ENDIF): ")]
    InvalidEnvelope,
    /// Does not have a valid name tag
    #[error("Invalid/Missing Batch tag")]
    InvalidBatchTag,
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

pub fn parse_inscription_data(
    script: &ScriptBuf,
    rollup_name: &str,
) -> Result<Vec<InscriptionBlob>, InscriptionParseError> {
    let mut instructions = script.instructions().peekable();
    let mut index = 0;
    let mut blobs = Vec::new();
    while enter_envelope(&mut instructions).is_ok() {
        blobs.push(parse_inscription_envelope(
            index,
            &mut instructions,
            rollup_name,
        )?);
        index += 1;
    }

    Ok(blobs)
}

/// Parse [`InscriptionData`]
///
/// # Errors
///
/// This function errors if it cannot parse the [`InscriptionData`]
pub fn parse_inscription_envelope(
    index: u32,
    instructions: &mut Peekable<Instructions<'_>>,
    rollup_name: &str,
) -> Result<InscriptionBlob, InscriptionParseError> {
    // Parse name
    if index == 0 {
        let name = next_bytes(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
        let extracted_rollup_name = String::from_utf8(name.to_vec())
            .map_err(|_| InscriptionParseError::InvalidNameValue)?;

        if extracted_rollup_name != rollup_name {
            return Err(InscriptionParseError::InvalidNameValue);
        }
    }
    let tag = next_int(instructions).ok_or(InscriptionParseError::InvalidFormat)?;

    let size = next_int(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
    let batch_data = extract_n_bytes(size, instructions)?;
    // DA for now but this should be based on the TAG on the script itself
    Ok(InscriptionBlob::new(
        BlobType::from_u32(tag).ok_or(InscriptionParseError::InvalidBatchTag)?,
        batch_data,
    ))
}

/// Check for consecutive `OP_FALSE` and `OP_IF` that marks the beginning of an inscription
fn enter_envelope(
    instructions: &mut Peekable<Instructions<'_>>,
) -> Result<(), InscriptionParseError> {
    // loop until OP_FALSE is found
    loop {
        let next = instructions.next();
        match next {
            None => {
                return Err(InscriptionParseError::InvalidEnvelope);
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
        return Err(InscriptionParseError::InvalidEnvelope);
    }
    Ok(())
}

/// Extract bytes of `size` from the remaining instructions
fn extract_n_bytes(
    size: u32,
    instructions: &mut Peekable<Instructions<'_>>,
) -> Result<Vec<u8>, InscriptionParseError> {
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
        Err(InscriptionParseError::InvalidBlob)
    }
}

#[cfg(test)]
mod tests {

    use strata_btcio::test_utils::generate_inscription_script_test;
    use strata_state::tx::BlobType;

    use super::*;

    #[test]
    fn test_parse_inscription_data() {
        let bytes = vec![0, 1, 2, 3];
        let inscription_data = vec![
            InscriptionBlob::new(BlobType::DA, bytes.clone()),
            InscriptionBlob::new(BlobType::Checkpoint, bytes.clone()),
        ];
        let script =
            generate_inscription_script_test(inscription_data.clone(), "TestRollup").unwrap();

        // Parse the rollup name
        let result = parse_inscription_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);
        // Try with larger size
        let bytes = vec![1; 2000];
        let inscription_data = vec![InscriptionBlob::new(BlobType::DA, bytes.clone())];
        let script =
            generate_inscription_script_test(inscription_data.clone(), "TestRollup").unwrap();

        // Parse the rollup name
        let result = parse_inscription_data(&script, "TestRollup").unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);
    }
}
