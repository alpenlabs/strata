use bitcoin::{
    blockdata::script,
    opcodes::{
        all::{OP_ENDIF, OP_IF},
        OP_FALSE,
    },
    script::PushBytesBuf,
    ScriptBuf,
};
use strata_primitives::l1::payload::{L1Payload, L1PayloadType};
use tracing::*;

use crate::envelope::parser::{BATCH_DATA_TAG, ROLLUP_NAME_TAG, VERSION_TAG};

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
        .push_slice(PushBytesBuf::try_from(ROLLUP_NAME_TAG.to_vec())?)
        .push_slice(PushBytesBuf::try_from(rollup_name.as_bytes().to_vec())?)
        .push_slice(PushBytesBuf::try_from(VERSION_TAG.to_vec())?)
        .push_slice(PushBytesBuf::from([version]))
        .push_slice(PushBytesBuf::try_from(BATCH_DATA_TAG.to_vec())?)
        .push_int(envelope_data.data().len() as i64);

    trace!(batchdata_size = %envelope_data.data().len(), "Inserting batch data");
    for chunk in envelope_data.data().chunks(520) {
        trace!(size=%chunk.len(), "inserting chunk");
        builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
    }
    builder = builder.push_opcode(OP_ENDIF);

    Ok(builder.into_script())
}

#[allow(dead_code)]
fn get_payload_type_tag(payload_type: &L1PayloadType) -> anyhow::Result<PushBytesBuf> {
    let ret = match *payload_type {
        L1PayloadType::Checkpoint => PushBytesBuf::try_from("checkpoint".as_bytes().to_vec())?,
        L1PayloadType::Da => PushBytesBuf::try_from("da".as_bytes().to_vec())?,
    };
    Ok(ret)
}
