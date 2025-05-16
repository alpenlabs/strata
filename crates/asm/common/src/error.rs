use thiserror::Error;

/// Errors that can occur while working with ASM subprotocols.
#[derive(Debug, Error)]
pub enum ASMError {
    /// The requested subprotocol ID was not found.
    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocol(u8),

    /// The requested subprotocol state ID was not found.
    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocolState(u8),

    /// Failed to deserialize the state of the given subprotocol.
    #[error("Failed to deserialize subprotocol {0:?} state")]
    Deserialization(u8, #[source] borsh::io::Error),

    /// Failed to serialize the state of the given subprotocol.
    #[error("Failed to serialize subprotocol {0:?} state")]
    Serialization(u8, #[source] borsh::io::Error),
}
