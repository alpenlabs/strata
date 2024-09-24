use bitcoin::{
    opcodes::all::OP_PUSHNUM_1,
    script::{Instruction, Instructions},
    Opcode,
};

/// Extract next instruction and try to parse it as an opcode
pub fn next_op(instructions: &mut Instructions<'_>) -> Option<Opcode> {
    let nxt = instructions.next();
    match nxt {
        Some(Ok(Instruction::Op(op))) => Some(op),
        _ => None,
    }
}

/// Extract next instruction and try to parse it as a byte slice
pub fn next_bytes<'a>(instructions: &mut Instructions<'a>) -> Option<&'a [u8]> {
    match instructions.next() {
        Some(Ok(Instruction::PushBytes(bytes))) => Some(bytes.as_bytes()),
        _ => None,
    }
}

/// Extract next integer value(unsigned)
pub fn next_int(instructions: &mut Instructions<'_>) -> Option<u32> {
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
            let opval = op.to_u8();
            let diff = opval - OP_PUSHNUM_1.to_u8();
            if (0..16).contains(&diff) {
                Some(diff as u32 + 1)
            } else {
                None
            }
        }
        _ => None,
    }
}
