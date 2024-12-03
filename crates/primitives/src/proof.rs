use serde::{Deserialize, Serialize};

use crate::buf::Buf32;

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollupVerifyingKey {
    // Verifying Key used to verify proof created using SP1
    SP1VerifyingKey(Buf32),
    // Verifying Key used to verify proof created using Risc0
    Risc0VerifyingKey(Buf32),
}
