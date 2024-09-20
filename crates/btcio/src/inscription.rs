use bitcoin::{
    blockdata::{
        opcodes::{
            all::{OP_ENDIF, OP_IF},
            OP_FALSE,
        },
        script,
    },
    script::PushBytesBuf,
    ScriptBuf,
};
use tracing::*;



#[derive(Debug, PartialEq)]
pub struct InscriptionData {
    rollup_name: String,
    batch_data: Vec<u8>,
    version: u8,
}

impl InscriptionData {
    pub const ROLLUP_NAME_TAG: &[u8] = &[1];
    pub const VERSION_TAG: &[u8] = &[2];
    pub const BATCH_DATA_TAG: &[u8] = &[3];

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

        debug!(batchdata_size = %self.batch_data.len(), "Inserting batch data");
        for chunk in self.batch_data.chunks(520) {
            debug!(size=%chunk.len(), "inserting chunk");
            builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
        }
        builder = builder.push_opcode(OP_ENDIF);

        Ok(builder.into_script())
    }
}
