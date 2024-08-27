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

pub(crate) struct InscriptionData {
    name: String,
    batchdata: Vec<u8>,
}

impl InscriptionData {
    const ROLLUP_NAME_TAG: &[u8] = &[1];
    const BATCH_DATA_TAG: &[u8] = &[2];
    // TODO: add other field, maybe a version

    pub fn new(name: String, batchdata: Vec<u8>) -> Self {
        Self { name, batchdata }
    }

    // TODO: return Result instead of Option
    pub fn from_script(_script: ScriptBuf) -> Option<Self> {
        None
    }

    // Generates a [`ScriptBuf`] that consists of OP_IF .. OP_ENDIF block
    pub fn to_script(&self) -> anyhow::Result<ScriptBuf> {
        let mut builder = script::Builder::new()
            .push_opcode(OP_FALSE)
            .push_opcode(OP_IF)
            .push_opcode(OP_IF)
            .push_slice(PushBytesBuf::try_from(Self::ROLLUP_NAME_TAG.to_vec())?)
            .push_slice(PushBytesBuf::try_from(self.name.as_bytes().to_vec())?)
            .push_slice(PushBytesBuf::try_from(Self::BATCH_DATA_TAG.to_vec())?)
            .push_int(self.batchdata.len() as i64);

        for chunk in self.batchdata.chunks(520) {
            builder = builder.push_slice(PushBytesBuf::try_from(chunk.to_vec())?);
        }
        builder = builder.push_opcode(OP_ENDIF);

        Ok(builder.into_script())
    }
}
