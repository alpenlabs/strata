#![allow(dead_code)]

use core::fmt;

use arbitrary::Arbitrary;
use borsh::{io, BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    bridge::OperatorIdx,
    buf::{Buf32, Buf64},
    prelude::BitcoinTxid,
};

/// Message container used to direct payloads depending on the context between parties.
///
/// # Caution
///
/// Users should not construct a [`BridgeMessage`] directly,
/// instead construct a [`MessageSigner`](super::util::MessageSigner) by
/// calling [`MessageSigner::new`](super::util::MessageSigner::new),
/// followed by [`sign_raw`](super::util::MessageSigner::sign_raw)
/// or [`sign_scope`](super::util::MessageSigner::sign_scope)
/// depending on the use case.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct BridgeMessage {
    /// Operator ID
    pub(crate) source_id: OperatorIdx,

    /// Schnorr signature of the message
    pub(crate) sig: Buf64,

    /// Purpose of the message.
    pub(crate) scope: Vec<u8>,

    /// Serialized message
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
        digest.update(self.source_id.to_be_bytes());
        digest.update((self.scope.len() as u64).to_be_bytes());
        digest.update(&self.scope);
        digest.update((self.payload.len() as u64).to_be_bytes());
        digest.update(&self.payload);

        let hash: [u8; 32] = digest.finalize().into();
        BridgeMsgId::from(Buf32::from(hash))
    }
}

impl TryFrom<Vec<u8>> for BridgeMessage {
    type Error = io::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value.as_ref())?;
        Ok(result)
    }
}

impl TryInto<Vec<u8>> for BridgeMessage {
    type Error = io::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        borsh::to_vec(&self)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Serialization error"))
    }
}

impl TryFrom<&[u8]> for BridgeMessage {
    type Error = io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value)?;
        Ok(result)
    }
}

impl TryFrom<Box<[u8]>> for BridgeMessage {
    type Error = io::Error;

    fn try_from(value: Box<[u8]>) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value.as_ref())?;
        Ok(result)
    }
}

impl TryInto<Box<[u8]>> for BridgeMessage {
    type Error = io::Error;

    fn try_into(self) -> Result<Box<[u8]>, Self::Error> {
        let serialized_vec = borsh::to_vec(&self)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Serialization error"))?;
        Ok(serialized_vec.into_boxed_slice()) // Convert Vec<u8> to Box<[u8]>
    }
}

/// Scope of the [`BridgeMessage`]
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub enum Scope {
    /// Used for debugging purposes.
    Misc,

    /// Deposit Signature with Outpoint.
    // TODO make this contain the outpoint
    V0DepositSig(BitcoinTxid),

    /// Deposit MuSig public nonce
    V0DepositPubNonce(BitcoinTxid),

    /// Withdrawal Signature with Deposit index.
    V0WithdrawalSig(BitcoinTxid),

    /// Withdrawal MuSig public nonce
    V0WithdrawalPubNonce(BitcoinTxid),
}

impl TryFrom<Vec<u8>> for Scope {
    type Error = io::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value.as_ref())?;
        Ok(result)
    }
}

impl TryInto<Vec<u8>> for Scope {
    type Error = io::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        borsh::to_vec(&self)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Serialization error"))
    }
}

impl TryFrom<&[u8]> for Scope {
    type Error = io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value)?;
        Ok(result)
    }
}

impl TryFrom<Box<[u8]>> for Scope {
    type Error = io::Error;

    fn try_from(value: Box<[u8]>) -> Result<Self, Self::Error> {
        let result = borsh::from_slice(value.as_ref())?;
        Ok(result)
    }
}

impl TryInto<Box<[u8]>> for Scope {
    type Error = io::Error;

    fn try_into(self) -> Result<Box<[u8]>, Self::Error> {
        let serialized_vec = borsh::to_vec(&self)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Serialization error"))?;
        Ok(serialized_vec.into_boxed_slice()) // Convert Vec<u8> to Box<[u8]>
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
pub struct RelayerConfig {
    /// Time we check for purgeable messages.
    pub refresh_interval: u64,

    /// Age after which we'll start to re-relay a message if we recv it again.
    pub stale_duration: u64,

    /// Relay misc messages that don't check signatures.
    pub relay_misc: bool,
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::ArbitraryGenerator;

    use super::{BridgeMessage, Scope};
    use crate::buf::Buf64;

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
