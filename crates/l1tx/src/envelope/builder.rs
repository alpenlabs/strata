use bitcoin::{
    blockdata::script,
    opcodes::{
        all::{OP_ENDIF, OP_IF},
        OP_FALSE,
    },
    script::PushBytesBuf,
    ScriptBuf,
};
use strata_state::da_blob::{L1Payload, L1PayloadType};
use tracing::*;

// Generates a [`ScriptBuf`] that consists of `OP_IF .. OP_ENDIF` block
pub fn build_envelope_script(
    envelope_data: &L1Payload,
    // TODO: get tagnames from config
    rollup_name: &str,
    version: u8,
) -> anyhow::Result<ScriptBuf> {
    let mut builder = script::Builder::new()
        .push_opcode(OP_FALSE)
        .push_opcode(OP_IF)
        .push_slice(get_payload_type_tag(envelope_data.payload_type())?);

    trace!(batchdata_size = %envelope_data.data().len(), "Inserting batch data");
    for chunk in envelope_data.data().chunks(520) {
        trace!(size=%chunk.len(), "inserting chunk");
        builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
    }
    builder = builder.push_opcode(OP_ENDIF);

    Ok(builder.into_script())
}

fn get_payload_type_tag(payload_type: &L1PayloadType) -> anyhow::Result<PushBytesBuf> {
    let ret = match *payload_type {
        L1PayloadType::Checkpoint => PushBytesBuf::try_from("checkpoint".as_bytes().to_vec())?,
        L1PayloadType::Da => PushBytesBuf::try_from("da".as_bytes().to_vec())?,
    };
    Ok(ret)
}
