use crate::buf::Buf32;

pub enum RollupVerifyingKey {
    SP1VerifyingKey(Buf32),
    Risc0VerifyingKey(Buf32),
}
