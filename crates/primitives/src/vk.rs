use borsh::{BorshDeserialize, BorshSerialize};
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

// TODO: move this somewhere else?
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum StrataProofId {
    BtcBlockspace(u64),
    EvmEeStf(u64),
    ClStf(u64),
    L1Batch(u64, u64),
    ClAgg(u64, u64),
    Checkpoint(u64),
}

#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum StrataHost {
    SP1,
    Risc0,
    Native,
}
