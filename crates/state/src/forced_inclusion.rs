//! Forced inclusion types.
//!
//! This is all stubs now so that we can define data structures later.

use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct ForcedInclusion {
    payload: Vec<u8>,
}

impl ForcedInclusion {
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}
