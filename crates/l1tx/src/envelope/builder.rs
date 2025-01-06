use bitcoin::{
    blockdata::script,
    opcodes::{
        all::{OP_ENDIF, OP_IF},
        OP_FALSE,
    },
    script::PushBytesBuf,
    ScriptBuf,
};
use strata_state::tx::EnvelopeData;
use tracing::*;

// TODO: remove these in favor of config
pub const ROLLUP_NAME_TAG: &[u8] = &[1];
pub const VERSION_TAG: &[u8] = &[2];
pub const BATCH_DATA_TAG: &[u8] = &[3];

// Generates a [`ScriptBuf`] that consists of `OP_IF .. OP_ENDIF` block
pub fn build_envelope_script(
    envelope_data: EnvelopeData,
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
        .push_int(envelope_data.batch_data().len() as i64);

    trace!(batchdata_size = %envelope_data.batch_data().len(), "Inserting batch data");
    for chunk in envelope_data.batch_data().chunks(520) {
        trace!(size=%chunk.len(), "inserting chunk");
        builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
    }
    builder = builder.push_opcode(OP_ENDIF);

    Ok(builder.into_script())
}
