use std::iter::Peekable;

use bitcoin::{
    opcodes::all::{OP_ENDIF, OP_IF},
    script::{Instruction, Instructions},
    Opcode, Script,
};
use strata_state::{batch::SignedBatchCheckpoint, tx::PayloadTypeTag};
use thiserror::Error;

use crate::visitor::OpVisitor;

#[derive(Debug, Error)]
pub enum EnvelopeParseError {
    /// Does not have an `OP_IF..OP_ENDIF` block
    #[error("Invalid/Missing envelope(NO OP_IF..OP_ENDIF): ")]
    InvalidEnvelope,
    /// Does not have a valid name tag
    #[error("Invalid/Missing Batch tag")]
    UnknownTag,
    /// Does not have a valid name value
    #[error("Invalid chunk")]
    InvalidChunk,
    /// Does not have a valid format
    #[error("Invalid Format")]
    InvalidFormat,
}

pub fn parse_reveal_script<'a>(
    script: &'a Script,
    da_tag: &str,
    ckpt_tag: &str,
    visitor: &mut impl OpVisitor<'a>,
) -> Result<(), EnvelopeParseError> {
    let mut instructions = script.instructions().peekable();
    while let Ok(envelope_iter) = extract_envelope(&mut instructions) {
        let mut envelope_iter = envelope_iter.peekable();
        let (tag, chunks) = parse_envelope(&mut envelope_iter, da_tag, ckpt_tag)?;
        match tag {
            PayloadTypeTag::Checkpoint => {
                let raw_checkpoint = chunks.flat_map(|chunk| chunk.to_vec()).collect::<Vec<_>>();
                let checkpoint_data = borsh::from_slice::<SignedBatchCheckpoint>(&raw_checkpoint)
                    .map_err(|_| EnvelopeParseError::InvalidChunk)?;
                visitor.visit_checkpoint(checkpoint_data);
            }
            PayloadTypeTag::DA => {
                visitor.visit_envelope_payload_chunks(chunks);
            }
        }
    }
    Ok(())
}

/// Iterates through [`Iterator<Item = Instruction>`]  to parse envelope
///
/// # Errors
/// This function errors if it is not the proper envelope format
pub fn parse_envelope<'a, 'b>(
    instructions: &'a mut impl Iterator<Item = Instruction<'b>>,
    da_tag: &str,
    ckpt_tag: &str,
) -> Result<(PayloadTypeTag, impl Iterator<Item = &'b [u8]> + 'a), EnvelopeParseError> {
    // get the payload_type
    let tag_str =
        std::str::from_utf8(next_bytes(instructions).ok_or(EnvelopeParseError::InvalidFormat)?)
            .map_err(|_| EnvelopeParseError::UnknownTag)?;

    let tag = get_payload_tag(tag_str, da_tag, ckpt_tag)?;
    let chunks = extract_bytes(instructions);

    Ok((tag, chunks))
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
fn extract_envelope<'a, 'b>(
    instructions: &'b mut Peekable<Instructions<'a>>,
) -> Result<impl Iterator<Item = Instruction<'a>> + 'b, EnvelopeParseError> {
    // Loop until OP_FALSE is found
    loop {
        let next = instructions.next();
        match next {
            None => return Err(EnvelopeParseError::InvalidEnvelope),
            // OP_FALSE is basically empty PushBytes
            Some(Ok(Instruction::PushBytes(bytes))) if bytes.as_bytes().is_empty() => break,
            _ => {
                // Just carry on until OP_FALSE is found
            }
        }
    }

    // Check if next opcode is OP_IF
    if next_op(instructions) != Some(OP_IF) {
        return Err(EnvelopeParseError::InvalidEnvelope);
    }

    // Return a custom iterator
    Ok(std::iter::from_fn(move || match instructions.next() {
        Some(Ok(Instruction::Op(OP_ENDIF))) => None,
        Some(Ok(instruction)) => Some(instruction),
        _ => None,
    }))
}

/// Extract bytes of `size` from the remaining instructions
fn extract_bytes<'c>(
    instructions: &mut impl Iterator<Item = Instruction<'c>>,
) -> impl Iterator<Item = &'c [u8]> + '_ {
    std::iter::from_fn(|| next_bytes(instructions))
}

fn next_bytes<'a>(instructions: &mut impl Iterator<Item = Instruction<'a>>) -> Option<&'a [u8]> {
    match instructions.next() {
        Some(Instruction::PushBytes(bytes)) => Some(bytes.as_bytes()),
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

    use strata_state::tx::{EnvelopePayload, PayloadTypeTag};

    use crate::{
        reveal::{builder::generate_envelope_script, parser::parse_reveal_script},
        visitor::OpVisitor,
    };

    struct MockOpVisitor {
        da: Vec<Vec<u8>>,
    }

    impl<'t> OpVisitor<'t> for MockOpVisitor {
        fn visit_deposit(&mut self, _: strata_state::tx::DepositInfo) {}

        fn visit_checkpoint(&mut self, _: strata_state::batch::SignedBatchCheckpoint) {}

        fn visit_envelope_payload_chunks(&mut self, val: impl Iterator<Item = &'t [u8]>) {
            self.da
                .push(val.flat_map(|x| x.iter().copied()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn test_parse_envelope_data() {
        let da_bytes = vec![0, 1, 2, 3];
        let envelope_data = vec![EnvelopePayload::new(PayloadTypeTag::DA, da_bytes.clone())];
        let script = generate_envelope_script(&envelope_data, "strata-da", "strata-ckpt").unwrap();
        let mut mock_visitor = MockOpVisitor { da: Vec::new() };
        // Try for da commitments
        let _ = parse_reveal_script(&script, "strata-da", "strata-ckpt", &mut mock_visitor);
        assert_eq!(da_bytes, mock_visitor.da[0]);

        let large_bytes = vec![0; 2000];
        // Try for da commitments
        let envelope_data = vec![EnvelopePayload::new(
            PayloadTypeTag::DA,
            large_bytes.clone(),
        )];
        let script = generate_envelope_script(&envelope_data, "strata-da", "strata-ckpt").unwrap();
        let mut mock_visitor = MockOpVisitor { da: Vec::new() };
        let _ = parse_reveal_script(&script, "strata-da", "strata-ckpt", &mut mock_visitor);
        assert_eq!(large_bytes, mock_visitor.da[0]);
    }
}
