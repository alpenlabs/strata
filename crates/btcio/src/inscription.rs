use bitcoin::{
    blockdata::{
        opcodes::{
            all::{OP_ENDIF, OP_IF},
            OP_FALSE,
        },
        script,
    },
    script::{Instruction, Instructions, PushBytesBuf},
    Opcode, ScriptBuf,
};
use thiserror::Error;

pub(crate) struct InscriptionData {
    name: String,
    batchdata: Vec<u8>,
}

impl InscriptionData {
    const ROLLUP_NAME_TAG: &[u8] = &[1];
    const VERSION_TAG: &[u8] = &[2];
    const BATCH_DATA_TAG: &[u8] = &[3];

    // NOTE: this may need to come from somewhere else rather than being hardcoded here
    const VERSION: u8 = 1;

    pub fn new(name: String, batchdata: Vec<u8>) -> Self {
        Self { name, batchdata }
    }

    // Generates a [`ScriptBuf`] that consists of `OP_IF .. OP_ENDIF` block
    pub fn to_script(&self) -> anyhow::Result<ScriptBuf> {
        let mut builder = script::Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(Self::ROLLUP_NAME_TAG.to_vec())?)
            .push_slice(PushBytesBuf::try_from(self.name.as_bytes().to_vec())?)
            .push_slice(PushBytesBuf::try_from(Self::VERSION_TAG.to_vec())?)
            .push_slice(PushBytesBuf::from([Self::VERSION]))
            .push_slice(PushBytesBuf::try_from(Self::BATCH_DATA_TAG.to_vec())?)
            .push_int(self.batchdata.len() as i64);

        for chunk in self.batchdata.chunks(520) {
            builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
        }
        builder = builder.push_opcode(OP_ENDIF);

        Ok(builder.into_script())
    }
}

#[derive(Debug, Error)]
pub enum InscriptionParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope")]
    InvalidEnvelope,
    /// Does not have a valid name tag
    #[error("Invalid/Missing name tag")]
    InvalidNameTag,
    /// Does not have a valid name value
    #[error("Invalid/Missing value")]
    InvalidNameValue,
    /// Does not have a valid blob tag
    #[error("Invalid/Missing blob tag")]
    InvalidBlobTag,
    /// Something else
    #[error("{0}")]
    Other(String),
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

        let tag = Self::next_bytes(&mut instructions);
        let name = Self::next_bytes(&mut instructions);

        match (tag, name) {
            (Some(tag), Some(namebytes)) if tag == InscriptionData::ROLLUP_NAME_TAG => {
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
                    return Err(InscriptionParseError::InvalidEnvelope);
                }
                // OP_FALSE is basically empty PushBytes
                Some(Ok(Instruction::PushBytes(bytes))) => {
                    if bytes.as_bytes().is_empty() {
                        break;
                    }
                }
                _ => {}
            }
        }

        // Check if next opcode is OP_IF
        let op_if = Self::next_op(instructions);
        if op_if != Some(OP_IF) {
            return Err(InscriptionParseError::InvalidEnvelope);
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
}

#[cfg(test)]
mod tests {
    use bitcoin::{blockdata::script::Builder, opcodes::OP_TRUE};

    use super::*;

    #[test]
    fn test_parse_rollup_name_valid() {
        // Create a valid inscription data
        let inscription_data = InscriptionData::new("TestRollup".to_string(), vec![0, 1, 2, 3]);
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
            Err(InscriptionParseError::InvalidEnvelope)
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
            Err(InscriptionParseError::InvalidEnvelope)
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
            Err(InscriptionParseError::InvalidEnvelope)
        ));
    }

    #[test]
    fn test_parse_rollup_name_missing_name_tag() {
        // Create a script that has OP_IF but no tag bytes
        let script = Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .into_script();
        println!("ScRIPT: {:?}", script.to_string());

        // Parse the rollup name
        let parser = InscriptionParser::new(script);
        let result = parser.parse_rollup_name();
        println!("{:?}", result);

        // Assert that it returns an InvalidNameTag error
        assert!(matches!(result, Err(InscriptionParseError::InvalidNameTag)));
    }
}
