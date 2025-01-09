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
    envelope_data: &L1Payload,
    version: u8,
) -> anyhow::Result<ScriptBuf> {
    let tag = get_payload_type_tag(envelope_data.payload_type(), params)?;
    let mut builder = script::Builder::new()
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(tag)
        // Insert version
        .push_slice(PushBytesBuf::from(version.to_be_bytes()))
        // Insert size
        .push_slice(PushBytesBuf::from(
            (envelope_data.data().len() as u32).to_be_bytes(),
        ));

    // Insert actual data
    trace!(batchdata_size = %envelope_data.data().len(), "Inserting batch data");
    for chunk in envelope_data.data().chunks(520) {
        trace!(size=%chunk.len(), "inserting chunk");
        builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
    }
    builder = builder.push_opcode(OP_ENDIF);

    Ok(builder.into_script())
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
