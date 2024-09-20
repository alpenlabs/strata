use bitcoin::{opcodes::all::OP_IF, script::{Instruction, Instructions},  ScriptBuf};
use tracing::debug;

use crate::inscription::InscriptionData;
use thiserror::Error;

use super::utils::{next_bytes, next_int, next_op};


#[derive(Debug, Error)]
pub enum InscriptionParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope: {0}")]
    InvalidEnvelope(String),
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


/// Parse [`InscriptionData`]
///
/// # Errors
///
/// This function errors if it cannot parse the [`InscriptionData`]
pub fn parse_inscription_data(script: ScriptBuf) -> Result<InscriptionData, InscriptionParseError> {
        let mut instructions = script.instructions();

        enter_envelope(&mut instructions)?;

        // Parse name
        let (tag, name) = parse_bytes_pair(&mut instructions)?;

        let rollup_name = match (tag.as_slice(), name) {
            (InscriptionData::ROLLUP_NAME_TAG, namebytes) => {
                String::from_utf8(namebytes).map_err(|_| InscriptionParseError::InvalidNameValue)
            }
            _ => Err(InscriptionParseError::InvalidNameTag),
        }?;

        // Parse version
        let (tag, ver) = parse_bytes_pair(&mut instructions)?;
        let version = match (tag.as_slice(), ver.as_slice()) {
            (InscriptionData::VERSION_TAG, [v]) => Ok(v),
            (InscriptionData::VERSION_TAG, _) => Err(InscriptionParseError::InvalidVersion),
            _ => Err(InscriptionParseError::InvalidVersionTag),
        }?;

        // Parse bytes
        let tag =
            next_bytes(&mut instructions).ok_or(InscriptionParseError::InvalidBlobTag)?;
        let size = next_int(&mut instructions);
        match (tag.as_slice(), size) {
            (InscriptionData::BATCH_DATA_TAG, Some(size)) => {
                let batch_data = extract_n_bytes(size, &mut instructions)?;
                Ok(InscriptionData::new(rollup_name, batch_data, *version))
            }
            (InscriptionData::BATCH_DATA_TAG, None) => Err(InscriptionParseError::InvalidBlob),
            _ => Err(InscriptionParseError::InvalidBlobTag),
        }
    }

    /// Check for consecutive `OP_FALSE` and `OP_IF` that marks the beginning of an inscription
    fn enter_envelope(instructions: &mut Instructions) -> Result<(), InscriptionParseError> {
        // loop until OP_FALSE is found
        loop {
            let next = instructions.next();
            match next {
                None => {
                    return Err(InscriptionParseError::InvalidEnvelope(
                        "No envelope found".to_string(),
                    ));
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
            return Err(InscriptionParseError::InvalidEnvelope(
                "Missing OP_IF".to_string(),
            ));
        }
        Ok(())
    }


    fn parse_bytes_pair(
        instructions: &mut Instructions,
    ) -> Result<(Vec<u8>, Vec<u8>), InscriptionParseError> {
        let tag = next_bytes(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
        let name = next_bytes(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
        Ok((tag, name))
    }


    /// Extract bytes of `size` from the remaining instructions
    fn extract_n_bytes(
        size: u32,
        instructions: &mut Instructions,
    ) -> Result<Vec<u8>, InscriptionParseError> {
        debug!("Extracting {} bytes from instructions", size);
        let mut data = vec![];
        let mut curr_size: u32 = 0;
        while let Some(bytes) = next_bytes(instructions) {
            data.extend_from_slice(&bytes);
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

    use crate::inscription::InscriptionData;
    use super::*;

    #[test]
    fn test_parse_inscription_data() {
        let bytes = vec![0, 1, 2, 3];
        let inscription_data = InscriptionData::new("TestRollup".to_string(), bytes.clone(), 1);
        let script = inscription_data
            .to_script()
            .expect("Failed to generate script");

        // Parse the rollup name
        let result = parse_inscription_data(script).unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);

        // Try with larger size
        let bytes = vec![1; 2000];
        let inscription_data = InscriptionData::new("TestRollup".to_string(), bytes.clone(), 1);
        let script = inscription_data
            .to_script()
            .expect("Failed to generate script");

        // Parse the rollup name
        let result = parse_inscription_data(script).unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);
    }
}
