use bitcoin::{
    blockdata::script,
    opcodes::{
        all::{OP_ENDIF, OP_IF},
        OP_FALSE,
    },
    script::PushBytesBuf,
    ScriptBuf,
};
use strata_primitives::{
    l1::payload::{L1Payload, L1PayloadType},
    params::Params,
};
use tracing::*;

// Generates a [`ScriptBuf`] that consists of `OP_IF .. OP_ENDIF` block
pub fn build_envelope_script(
    params: &Params,
    payloads: &[L1Payload],
    version: u8,
) -> anyhow::Result<ScriptBuf> {
    let mut bytes = Vec::new();
    for payload in payloads {
        let script_bytes = build_payload_envelope(params, payload, version)?;
        bytes.extend(script_bytes);
    }
    Ok(ScriptBuf::from_bytes(bytes))
}

fn build_payload_envelope(
    params: &Params,
    payload: &L1Payload,
    version: u8,
) -> anyhow::Result<Vec<u8>> {
    let tag = get_payload_type_tag(payload.payload_type(), params)?;
    let mut builder = script::Builder::new()
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(tag)
        // Insert version
        .push_slice(PushBytesBuf::from(version.to_be_bytes()))
        // Insert size
        .push_slice(PushBytesBuf::from(
            (payload.data().len() as u32).to_be_bytes(),
        ));

    // Insert actual data
    trace!(batchdata_size = %payload.data().len(), "Inserting batch data");
    for chunk in payload.data().chunks(520) {
        trace!(size=%chunk.len(), "inserting chunk");
        builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
    }
    builder = builder.push_opcode(OP_ENDIF);
    Ok(builder.as_bytes().to_vec())
}

fn get_payload_type_tag(
    payload_type: &L1PayloadType,
    params: &Params,
) -> anyhow::Result<PushBytesBuf> {
    Ok(match *payload_type {
        L1PayloadType::Checkpoint => {
            PushBytesBuf::try_from(params.rollup().checkpoint_tag.as_bytes().to_vec())?
        }
        L1PayloadType::Da => PushBytesBuf::try_from(params.rollup().da_tag.as_bytes().to_vec())?,
    })
}
