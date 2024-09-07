use bitcoin::{
    blockdata::{
        opcodes::{
            all::{OP_ENDIF, OP_IF},
            OP_FALSE,
        },
        script,
    },
    opcodes::all::*,
    script::{Instruction, Instructions, PushBytesBuf},
    Opcode, ScriptBuf,
};
use thiserror::Error;
use tracing::*;

#[derive(Debug, PartialEq)]
pub struct InscriptionData {
    rollup_name: String,
    batch_data: Vec<u8>,
    version: u8,
}

impl InscriptionData {
    const ROLLUP_NAME_TAG: &[u8] = &[1];
    const VERSION_TAG: &[u8] = &[2];
    const BATCH_DATA_TAG: &[u8] = &[3];

    pub fn new(rollup_name: String, batch_data: Vec<u8>, version: u8) -> Self {
        Self {
            rollup_name,
            batch_data,
            version,
        }
    }

    pub fn batch_data(&self) -> &[u8] {
        &self.batch_data
    }

    // Generates a [`ScriptBuf`] that consists of `OP_IF .. OP_ENDIF` block
    pub fn to_script(&self) -> anyhow::Result<ScriptBuf> {
        let mut builder = script::Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(Self::ROLLUP_NAME_TAG.to_vec())?)
            .push_slice(PushBytesBuf::try_from(
                self.rollup_name.as_bytes().to_vec(),
            )?)
            .push_slice(PushBytesBuf::try_from(Self::VERSION_TAG.to_vec())?)
            .push_slice(PushBytesBuf::from([self.version]))
            .push_slice(PushBytesBuf::try_from(Self::BATCH_DATA_TAG.to_vec())?)
            .push_int(self.batch_data.len() as i64);

        for chunk in self.batch_data.chunks(520) {
            println!(
                "inserting chunk of size {:?} {:?}",
                self.batch_data.len(),
                chunk
            );
            builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
        }
        builder = builder.push_opcode(OP_ENDIF);

        Ok(builder.into_script())
    }
}

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

/// Parser for relevant inscription data from a script.
/// This expects a specific structure of inscription data.
pub struct InscriptionParser {
    script: ScriptBuf,
    // NOTE: might need to keep track of the script iterator
}

impl InscriptionParser {
    pub fn new(script: ScriptBuf) -> Self {
        Self { script }
    }

    /// Parse the rollup name
    ///
    /// # Errors
    ///
    /// This function errors if no rollup name is found in the [`InscriptionData`]
    pub fn parse_rollup_name(&self) -> Result<String, InscriptionParseError> {
        let mut instructions = self.script.instructions();

        Self::enter_envelope(&mut instructions)?;

        let (tag, name) = Self::parse_tag_value(&mut instructions)?;

        match (tag.as_slice(), name) {
            (InscriptionData::ROLLUP_NAME_TAG, namebytes) => {
                String::from_utf8(namebytes).map_err(|_| InscriptionParseError::InvalidNameValue)
            }
            _ => Err(InscriptionParseError::InvalidNameTag),
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
                    return Err(InscriptionParseError::InvalidEnvelope(
                        "No OP_FALSE found".to_string(),
                    ))
                }
            }
        }

        // Check if next opcode is OP_IF
        let op_if = Self::next_op(instructions);
        if op_if != Some(OP_IF) {
            return Err(InscriptionParseError::InvalidEnvelope(
                "Missing OP_IF".to_string(),
            ));
        }
        Ok(())
    }

    /// Extract next instruction and try to parse it as an opcode
    fn next_op(instructions: &mut Instructions) -> Option<Opcode> {
        let nxt = instructions.next();
        match nxt {
            Some(Ok(Instruction::Op(op))) => Some(op),
            _ => None,
        }
    }

    /// Extract next instruction and try to parse it as bytes
    fn next_bytes(instructions: &mut Instructions) -> Option<Vec<u8>> {
        match instructions.next() {
            Some(Ok(Instruction::PushBytes(bytes))) => Some(bytes.as_bytes().to_vec()),
            _ => None,
        }
    }

    fn next_size(instructions: &mut Instructions) -> Option<u32> {
        let n = instructions.next();
        match n {
            Some(Ok(Instruction::PushBytes(bytes))) => {
                // Convert the bytes to an integer
                if bytes.len() > 4 {
                    return None;
                }
                let mut buf = [0; 4];
                buf[..bytes.len()].copy_from_slice(bytes.as_bytes());
                Some(u32::from_le_bytes(buf))
            }
            Some(Ok(Instruction::Op(op))) => {
                // Handle small integers pushed by OP_1 to OP_16
                if op == OP_PUSHNUM_1 {
                    Some(1)
                } else if op == OP_PUSHNUM_2 {
                    Some(2)
                } else if op == OP_PUSHNUM_3 {
                    Some(3)
                } else if op == OP_PUSHNUM_4 {
                    Some(4)
                } else if op == OP_PUSHNUM_5 {
                    Some(5)
                } else if op == OP_PUSHNUM_6 {
                    Some(6)
                } else if op == OP_PUSHNUM_7 {
                    Some(7)
                } else if op == OP_PUSHNUM_8 {
                    Some(8)
                } else if op == OP_PUSHNUM_9 {
                    Some(9)
                } else if op == OP_PUSHNUM_10 {
                    Some(10)
                } else if op == OP_PUSHNUM_11 {
                    Some(11)
                } else if op == OP_PUSHNUM_12 {
                    Some(12)
                } else if op == OP_PUSHNUM_13 {
                    Some(13)
                } else if op == OP_PUSHNUM_14 {
                    Some(14)
                } else if op == OP_PUSHNUM_15 {
                    Some(15)
                } else if op == OP_PUSHNUM_16 {
                    Some(16)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_tag_value(
        instructions: &mut Instructions,
    ) -> Result<(Vec<u8>, Vec<u8>), InscriptionParseError> {
        let tag = Self::next_bytes(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
        let name = Self::next_bytes(instructions).ok_or(InscriptionParseError::InvalidFormat)?;
        Ok((tag, name))
    }

    /// Parse [`InsriptionData`]
    ///
    /// # Errors
    ///
    /// This function errors if it cannot parse the [`InscriptionData`]
    pub fn parse_inscription_data(&self) -> Result<InscriptionData, InscriptionParseError> {
        let mut instructions = self.script.instructions();

        Self::enter_envelope(&mut instructions)?;

        // parse name
        let (tag, name) = Self::parse_tag_value(&mut instructions)?;

        let rollup_name = match (tag.as_slice(), name) {
            (InscriptionData::ROLLUP_NAME_TAG, namebytes) => {
                String::from_utf8(namebytes).map_err(|_| InscriptionParseError::InvalidNameValue)
            }
            _ => Err(InscriptionParseError::InvalidNameTag),
        }?;

        // parse version
        let (tag, ver) = Self::parse_tag_value(&mut instructions)?;
        let version = match (tag.as_slice(), ver.as_slice()) {
            (InscriptionData::VERSION_TAG, [v]) => Ok(v),
            (InscriptionData::VERSION_TAG, _) => Err(InscriptionParseError::InvalidVersion),
            _ => Err(InscriptionParseError::InvalidVersionTag),
        }?;

        // parse bytes
        let tag =
            Self::next_bytes(&mut instructions).ok_or(InscriptionParseError::InvalidBlobTag)?;
        let size = Self::next_size(&mut instructions);
        match (tag.as_slice(), size) {
            (InscriptionData::BATCH_DATA_TAG, Some(size)) => {
                let batch_data = Self::extract_n_bytes(size, &mut instructions)?;
                Ok(InscriptionData::new(rollup_name, batch_data, *version))
            }
            (InscriptionData::BATCH_DATA_TAG, None) => Err(InscriptionParseError::InvalidBlob),
            _ => Err(InscriptionParseError::InvalidBlobTag),
        }
    }

    fn extract_n_bytes(
        size: u32,
        instructions: &mut Instructions,
    ) -> Result<Vec<u8>, InscriptionParseError> {
        debug!("Extracting {} bytes from instructions", size);
        let mut data = vec![];
        let mut curr_size: u32 = 0;
        while let Some(bytes) = Self::next_bytes(instructions) {
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
}

#[cfg(test)]
mod tests {
    use bitcoin::{blockdata::script::Builder, opcodes::OP_TRUE};

    use super::*;

    #[test]
    fn test_parse_rollup_name_valid() {
        // Create a valid inscription data
        let inscription_data = InscriptionData::new("TestRollup".to_string(), vec![0, 1, 2, 3], 1);
        let script = inscription_data
            .to_script()
            .expect("Failed to generate script");

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert the rollup name was parsed correctly
        assert_eq!(result.unwrap(), "TestRollup");
    }

    #[test]
    fn test_parse_rollup_name_invalid_envelope() {
        // Create an invalid script without OP_IF
        let script = Builder::new()
            .push_opcode(OP_FALSE)
            .push_slice(PushBytesBuf::try_from(InscriptionData::ROLLUP_NAME_TAG.to_vec()).unwrap())
            .push_slice(PushBytesBuf::try_from("TestRollup".as_bytes().to_vec()).unwrap())
            .into_script();

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert that it returns an InvalidEnvelope error
        assert!(matches!(
            result,
            Err(InscriptionParseError::InvalidEnvelope(_))
        ));

        // Create an invalid script without OP_FALSE
        let script = Builder::new()
            .push_opcode(OP_TRUE)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(InscriptionData::ROLLUP_NAME_TAG.to_vec()).unwrap())
            .push_slice(PushBytesBuf::try_from("TestRollup".as_bytes().to_vec()).unwrap())
            .into_script();

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert that it returns an InvalidEnvelope error
        assert!(matches!(
            result,
            Err(InscriptionParseError::InvalidEnvelope(_))
        ));
    }

    #[test]
    fn test_parse_rollup_name_invalid_name_tag() {
        // Create a script with an incorrect name tag
        let script = Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(vec![9]).unwrap()) // Invalid tag
            .push_slice(PushBytesBuf::try_from("TestRollup".as_bytes().to_vec()).unwrap())
            .into_script();

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert that it returns an InvalidNameTag error
        assert!(matches!(result, Err(InscriptionParseError::InvalidNameTag)));
    }

    #[test]
    fn test_parse_rollup_name_invalid_utf8() {
        // Create a script with invalid UTF-8 for the name
        let script = Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(InscriptionData::ROLLUP_NAME_TAG.to_vec()).unwrap())
            .push_slice(PushBytesBuf::try_from(vec![0xFF, 0xFF, 0xFF]).unwrap()) // Invalid UTF-8 bytes
            .into_script();

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert that it returns an InvalidNameValue error
        assert!(matches!(
            result,
            Err(InscriptionParseError::InvalidNameValue)
        ));
    }

    #[test]
    fn test_parse_rollup_name_missing_name_bytes() {
        // Create a script that omits the rollup name bytes
        let script = Builder::new()
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(InscriptionData::ROLLUP_NAME_TAG.to_vec()).unwrap())
            .into_script();

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();

        // Assert that it returns an InvalidEnvelope error
        assert!(matches!(
            result,
            Err(InscriptionParseError::InvalidEnvelope(_))
        ));
    }

    #[test]
    fn test_parse_inscription_data() {
        let bytes = vec![0, 1, 2, 3];
        let inscription_data = InscriptionData::new("TestRollup".to_string(), bytes.clone(), 1);
        let script = inscription_data
            .to_script()
            .expect("Failed to generate script");

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_inscription_data().unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);

        // Try with larger size
        let bytes = vec![1; 2000];
        let inscription_data = InscriptionData::new("TestRollup".to_string(), bytes.clone(), 1);
        let script = inscription_data
            .to_script()
            .expect("Failed to generate script");

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_inscription_data().unwrap();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, inscription_data);
    }
}
