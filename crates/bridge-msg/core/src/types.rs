#![allow(dead_code)]

use core::fmt;

use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Message container used to direct payloads depending on the context between parties.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct BridgeMessage {
    /// Operator ID
    pub(crate) source_id: u32,

    /// Schnorr signature of the message
    pub(crate) sig: Buf64,

    /// Purpose of the message.
    pub(crate) scope: Vec<u8>,

    /// serialized message
    pub(crate) payload: Vec<u8>,
}

impl<'a> Arbitrary<'a> for BridgeMessage {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let source_id = u32::arbitrary(u)?;
        let sig = Buf64::arbitrary(u)?;
        let scope = borsh::to_vec(&Scope::Misc).unwrap();
        let mut payload = vec![0; 20];
        u.fill_buffer(&mut payload)?;

        Ok(Self {
            source_id,
            sig,
            scope,
            payload,
        })
    }
}

impl BridgeMessage {
    /// Source ID.
    pub fn source_id(&self) -> u32 {
        self.source_id
    }

    /// Signature.
    pub fn signature(&self) -> &Buf64 {
        &self.sig
    }

    /// Raw scope.
    pub fn scope(&self) -> &[u8] {
        &self.scope
    }

    /// Raw payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Tries to parse the scope buf as a typed scope.
    pub fn try_parse_scope(&self) -> Option<Scope> {
        Scope::try_from_slice(self.scope()).ok()
    }

    /// Computes a msg ID based on the .
    pub fn compute_id(&self) -> BridgeMsgId {
        // No signature because it might be malleable and it doesn't have any
        // useful data in it we'd want to inspect.
        let mut digest = Sha256::default();
        digest.update(&self.source_id.to_be_bytes());
        digest.update(&(self.scope.len() as u64).to_be_bytes());
        digest.update(&self.scope);
        digest.update(&(self.payload.len() as u64).to_be_bytes());
        digest.update(&self.payload);

        let hash: [u8; 32] = digest.finalize().into();
        BridgeMsgId::from(Buf32::from(hash))
    }
}

/// Scope of the [`BridgeMessage`]
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub enum Scope {
    /// Used for debugging purposes.
    Misc,

    /// Deposit Signature with Outpoint.
    // TODO make this contain the outpoint
    V0DepositSig(u32),

    /// Withdrawal Signature with Deposit index.
    V0WithdrawalSig(u32),
}

impl Scope {
    /// Tries to parse the scope from a slice.
    pub fn try_from_slice(raw: &[u8]) -> anyhow::Result<Scope> {
        Ok(borsh::from_slice(raw)?)
    }
}

/// ID of a [``BridgeMessage``] computed from the sender ID, scope, and payload.
#[derive(
    Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Arbitrary, BorshDeserialize, BorshSerialize,
)]
pub struct BridgeMsgId(Buf32);

impl BridgeMsgId {
    pub fn inner(&self) -> &Buf32 {
        &self.0
    }

    pub fn into_inner(self) -> Buf32 {
        self.0
    }
}

impl From<Buf32> for BridgeMsgId {
    fn from(value: Buf32) -> Self {
        Self(value)
    }
}

impl From<BridgeMsgId> for Buf32 {
    fn from(value: BridgeMsgId) -> Self {
        value.0
    }
}

impl AsRef<[u8; 32]> for BridgeMsgId {
    fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

impl fmt::Debug for BridgeMsgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for BridgeMsgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Copy, Clone, Deserialize, Debug)]
pub struct BridgeConfig {
    // Time for which message is stored and bandwidth is reset
    pub refresh_interval: u64,

    // max no. of messages corresponding to single operator.
    pub bandwidth: u32,
}

#[cfg(test)]
mod tests {
    use alpen_express_primitives::buf::{Buf32, Buf64};
    use alpen_test_utils::ArbitraryGenerator;

    use super::{BridgeMessage, Scope};
    use crate::types::BridgeMsgId;

    fn get_arb_bridge_msg() -> BridgeMessage {
        let msg: BridgeMessage = ArbitraryGenerator::new().generate();
        msg
    }

    fn make_bridge_msg() -> BridgeMessage {
        BridgeMessage {
            source_id: 1,
            sig: Buf64::from([0; 64]),
            scope: borsh::to_vec(&Scope::Misc).unwrap(),
            payload: vec![1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn test_get_scope_raw() {
        let msg = make_bridge_msg();

        assert_eq!(msg.scope(), vec![0])
    }
}
