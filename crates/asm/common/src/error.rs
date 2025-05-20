use thiserror::Error;

use crate::SubprotocolId;

/// Errors that can occur while working with ASM subprotocols.
#[derive(Debug, Error)]
pub enum AsmError {
    /// Subprotocol ID of a decoded section did not match the expected subprotocol ID.
    #[error("tried to decode section of ID {0} as ID {1}")]
    SubprotoIdMismatch(SubprotocolId, SubprotocolId),

    /// The requested subprotocol ID was not found.
    #[error("subproto {0:?} does not exist")]
    InvalidSubprotocol(SubprotocolId),

    /// The requested subprotocol state ID was not found.
    #[error("subproto {0:?} does not exist")]
    InvalidSubprotocolState(SubprotocolId),

    /// Failed to deserialize the state of the given subprotocol.
    #[error("failed to deserialize subprotocol {0} state: {1}")]
    Deserialization(SubprotocolId, #[source] borsh::io::Error),

    /// Failed to serialize the state of the given subprotocol.
    #[error("failed to serialize subprotocol {0} state: {1}")]
    Serialization(SubprotocolId, #[source] borsh::io::Error),
}
