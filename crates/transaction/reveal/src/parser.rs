use std::iter::Peekable;

use bitcoin::{
    opcodes::all::OP_IF,
    script::{Instruction, Instructions},
    Opcode, Script,
};
use strata_state::tx::{EnvelopePayload, PayloadTypeTag};
use thiserror::Error;

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
    /// Does not have a valid format
    #[error("Invalid Format")]
    InvalidFormat,
}

/// EnvelopeData doesn't own the chunks it contains. EnvelopePayload owns it
#[derive(Debug)]
pub struct EnvelopeData<'t> {
    pub tag: PayloadTypeTag,
    pub chunks: Vec<&'t [u8]>,
}

impl<'t> EnvelopeData<'t> {
    /// Converts non contiguous chunks into a one flattened chunk
    pub fn get_flattened_chunks(&self) -> Vec<u8> {
        self.chunks
            .iter()
            .copied()
            .flatten()
            .copied()
            .collect::<Vec<u8>>()
    }

    /// Converts into owned counterpart
    pub fn to_envelope_payload(&self) -> EnvelopePayload {
        EnvelopePayload::new(self.tag, self.get_flattened_chunks())
    }
}

impl<'t> EnvelopeData<'t> {
    fn new(tag: PayloadTypeTag, chunks: Vec<&'t [u8]>) -> Self {
        Self { tag, chunks }
    }
}

pub fn parse_script_for_envelope<'a>(
    script: &'a Script,
    da_tag: &str,
    ckpt_tag: &str,
) -> Result<Vec<EnvelopeData<'a>>, EnvelopeParseError> {
    let mut instructions = script.instructions().peekable();
    let mut blobs = Vec::new();
    while enter_envelope(&mut instructions).is_ok() {
        blobs.push(parse_envelope(&mut instructions, da_tag, ckpt_tag)?);
    }

    Ok(blobs)
}

/// To Parse [`EnvelopePayload`]
///
/// # Errors
///
/// This function errors if it cannot extract the [`EnvelopePayload`] from the bitcoin instructions.
pub fn parse_envelope<'b, 'c>(
    instructions: &mut Peekable<Instructions<'c>>,
    da_tag: &'b str,
    ckpt_tag: &'b str,
) -> Result<EnvelopeData<'c>, EnvelopeParseError> {
    // get the payload_type
    let tag_str =
        std::str::from_utf8(next_bytes(instructions).ok_or(EnvelopeParseError::InvalidFormat)?)
            .map_err(|_| EnvelopeParseError::InvalidNameValue)?;

    let tag = get_payload_tag(tag_str, da_tag, ckpt_tag)?;

    let batch_data = extract_bytes(instructions)?;

    Ok(EnvelopeData::new(tag, batch_data))
}

fn get_payload_tag(
    cur_tag: &str,
    da_tag: &str,
    ckpt_tag: &str,
) -> Result<PayloadTypeTag, EnvelopeParseError> {
    match cur_tag {
        tag if tag == da_tag => Ok(PayloadTypeTag::DA),
        tag if tag == ckpt_tag => Ok(PayloadTypeTag::Checkpoint),
        _ => Err(EnvelopeParseError::UnknownTag),
    }
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
fn extract_bytes<'c>(
    instructions: &mut Peekable<Instructions<'c>>,
) -> Result<Vec<&'c [u8]>, EnvelopeParseError> {
    let mut data = vec![];
    while let Some(bytes) = next_bytes(instructions) {
        data.push(bytes)
    }
    Ok(data)
}

fn next_bytes<'a>(instructions: &mut Peekable<Instructions<'a>>) -> Option<&'a [u8]> {
    match instructions.next() {
        Some(Ok(Instruction::PushBytes(bytes))) => Some(bytes.as_bytes()),
        _ => None,
    }
}

fn next_op(instructions: &mut Peekable<Instructions<'_>>) -> Option<Opcode> {
    let nxt = instructions.next();
    match nxt {
        Some(Ok(Instruction::Op(op))) => Some(op),
        _ => None,
    }
}

#[cfg(test)]
mod tests {

    use strata_state::tx::PayloadTypeTag;

    use super::*;
    use crate::builder::generate_envelope_script;

    #[test]
    fn test_parse_envelope_data() {
        let bytes = vec![0, 1, 2, 3];
        let envelope_data = vec![
            EnvelopePayload::new(PayloadTypeTag::DA, bytes.clone()),
            EnvelopePayload::new(PayloadTypeTag::Checkpoint, bytes.clone()),
        ];
        let script = generate_envelope_script(&envelope_data, "strata-da", "strata-ckpt").unwrap();

        // Parse the result
        let result = parse_script_for_envelope(&script, "strata-da", "strata-ckpt")
            .unwrap()
            .iter()
            .map(|res| res.to_envelope_payload())
            .collect::<Vec<EnvelopePayload>>();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);

        // Try with larger size
        let bytes = vec![1; 2000];
        let envelope_data = vec![EnvelopePayload::new(PayloadTypeTag::DA, bytes.clone())];
        let script = generate_envelope_script(&envelope_data, "strata-da", "strata-ckpt").unwrap();
        let result = parse_script_for_envelope(&script, "strata-da", "strata-ckpt")
            .unwrap()
            .iter()
            .map(|res| res.to_envelope_payload())
            .collect::<Vec<EnvelopePayload>>();

        // Assert the rollup name was parsed correctly
        assert_eq!(result, envelope_data);
    }
}
