use thiserror::Error;

#[derive(Debug, Error)]
pub enum ASMError {
    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocol(u8),

    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocolState(u8),

    #[error("Failed to deserialize subprotocol {0:?} state")]
    Deserialization(u8, #[source] borsh::io::Error),

    #[error("Failed to serialize subprotocol {0:?} state")]
    Serialization(u8, #[source] borsh::io::Error),
}
