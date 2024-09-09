#![allow(dead_code)]

use core::fmt;

use alpen_express_primitives::buf::{Buf32, Buf64};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Bridge Message to be passed by the operator for Signature Duties
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct BridgeMessage {
    /// Operator Id
    pub(crate) source_id: u32,
    /// Schnorr signature of the message
    pub(crate) sig: Buf64,
    /// Purpose of the message.
    pub(crate) scope: Scope,
    /// serialized message
    pub(crate) payload: Vec<u8>,
}

impl<'a> Arbitrary<'a> for BridgeMessage {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let source_id = u32::arbitrary(u)?;
        let sig = Buf64::arbitrary(u)?;
        let scope = Scope::arbitrary(u)?;
        let mut payload = Vec::with_capacity(20);
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
    /// computes the [`BridgeMsgId`] by serializing and then hashing the [`BridgeMessage`]
    pub fn compute_id(&self) -> anyhow::Result<BridgeMsgId> {
        // Serialize the BridgeMessage struct into a binary format using bincode
        let mut serialized_data: Vec<u8> = Vec::new();
        BorshSerialize::serialize(&self, &mut serialized_data)?;
        let hash = Sha256::digest(&serialized_data);
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);

        Ok(BridgeMsgId::from(Buf32::from(result)))
    }

    /// get the deserialized Scope in enum form
    pub fn get_scope(&self) -> &Scope {
        &self.scope
    }

    /// get the scope in raw form
    pub fn get_scope_raw(&self) -> anyhow::Result<Vec<u8>> {
        let mut writer = Vec::new();
        BorshSerialize::serialize(&self.scope, &mut writer)?;
        Ok(writer)
    }

    /// raw payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// signature
    pub fn signature(&self) -> &Buf64 {
        &self.sig
    }

    /// source id
    pub fn source_id(&self) -> u32 {
        self.source_id
    }
}

/// Scope of the [`BridgeMessage`]
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Deserialize, Serialize,
)]
pub enum Scope {
    /// Deposit Signature with Outpoint
    V0DepositSig(u32),
    // Withdrawal Signature with Deposit index. Withdrawal are related to Deposits
    V0WithdrawalSig(u32),
}

impl Scope {
    /// get the deserialized Scope in enum form
    pub fn from_raw(raw: &[u8]) -> anyhow::Result<Scope> {
        let scope: Scope = Scope::try_from_slice(raw)?;

        Ok(scope)
    }
}

/// Id of [`BridgeMessage`]
#[derive(Clone, Eq, PartialEq, Hash, Arbitrary)]
pub struct BridgeMsgId(Buf32);

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
pub struct BridgeParams {
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

    fn get_bridge_msg() -> BridgeMessage {
        let scope = Scope::V0DepositSig(10);

        BridgeMessage {
            source_id: 1,
            sig: Buf64::from([0; 64]),
            scope,
            payload: vec![1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn test_compute_id() {
        let msg = get_bridge_msg();

        let msg_id = BridgeMsgId::from(Buf32::from([
            0xf7, 0xcd, 0xde, 0x84, 0x01, 0x35, 0x2e, 0x70, 0x92, 0x08, 0x69, 0x1a, 0x13, 0xcd,
            0x02, 0x79, 0xfc, 0x12, 0x71, 0xdd, 0xd1, 0xf7, 0x4f, 0xb1, 0xe6, 0x12, 0xec, 0xbc,
            0xf2, 0xa0, 0x2f, 0xc7,
        ]));

        assert_eq!(msg.compute_id().unwrap(), msg_id);
    }

    #[test]
    fn test_get_scope_raw() {
        let msg = get_bridge_msg();

        assert_eq!(msg.get_scope_raw().unwrap(), vec![0, 10, 0, 0, 0])
    }
}
