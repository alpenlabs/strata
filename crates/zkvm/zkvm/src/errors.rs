use serde::{de::Error as SerdeDeError, ser::Error as SerdeSerError};
use thiserror::Error;

pub type ZkVmResult<T> = Result<T, ZkVmError>;

/// Represents different types of errors that can occur in the ZkVM system
#[derive(Debug, Error)]
pub enum ZkVmError {
    /// Error during serialization of input or output
    #[error("Serialization failed")]
    SerializationError {
        /// Specific serialization error type
        #[source]
        source: SerializationErrorSource,
    },

    /// Error during deserialization of input or output
    #[error("Deserialization failed")]
    DeserializationError {
        /// Specific deserialization error type
        #[source]
        source: DeserializationErrorSource,
    },
    /// Error during proof generation
    #[error("Proof generation failed")]
    ProofGenerationError(String),

    /// Error during proof verification
    #[error("Proof verification failed")]
    ProofVerificationError(String),

    /// Input-related errors
    #[error("Input validation failed")]
    InputError(String),

    /// ELF-related errors
    #[error("ELF validation failed")]
    InvalidELF(String),

    /// Verification Key related errors
    #[error("Invalid Verification Key")]
    InvalidVerificationKey,

    /// Generic error for other cases
    #[error("An unexpected error occurred")]
    Other(String),
}

/// Enum to statically handle different serialization error sources
#[derive(Debug, Error)]
pub enum SerializationErrorSource {
    /// Bincode serialization error
    #[error("Bincode serialization error")]
    Bincode(#[from] bincode::Error),

    /// Borsh serialization error
    #[error("Borsh serialization error")]
    Borsh(#[from] borsh::io::Error),

    /// Serde serialization error
    #[error("Serde serialization error: {0}")]
    Serde(String),

    /// Other serialization errors
    #[error("Other serialization error: {0}")]
    Other(String),
}

/// Enum to statically handle different deserialization error sources
#[derive(Debug, Error)]
pub enum DeserializationErrorSource {
    /// Bincode deserialization error
    #[error("Bincode deserialization error")]
    Bincode(#[from] bincode::Error),

    /// Borsh deserialization error
    #[error("Borsh deserialization error")]
    Borsh(#[from] borsh::io::Error),

    /// Serde deserialization error
    #[error("Serde deserialization error: {0}")]
    Serde(String),

    /// Other deserialization errors
    #[error("Other deserialization error: {0}")]
    Other(String),
}

/// Trait for converting Serde serialization errors
pub trait SerdeSerErrorConvert {
    /// Convert the Serde serialization error to ZkVmError
    fn to_zkvm_error(self) -> ZkVmError;
}

/// Trait for converting Serde deserialization errors
pub trait SerdeDeErrorConvert {
    /// Convert the Serde deserialization error to ZkVmError
    fn to_zkvm_error(self) -> ZkVmError;
}

// Implement conversion traits for any Serde serializer error
impl<E> SerdeSerErrorConvert for E
where
    E: SerdeSerError,
{
    fn to_zkvm_error(self) -> ZkVmError {
        ZkVmError::SerializationError {
            source: SerializationErrorSource::Serde(self.to_string()),
        }
    }
}

// Implement conversion traits for any Serde deserializer error
impl<E> SerdeDeErrorConvert for E
where
    E: SerdeDeError,
{
    fn to_zkvm_error(self) -> ZkVmError {
        ZkVmError::DeserializationError {
            source: DeserializationErrorSource::Serde(self.to_string()),
        }
    }
}

impl From<borsh::io::Error> for ZkVmError {
    fn from(err: borsh::io::Error) -> Self {
        ZkVmError::SerializationError {
            source: SerializationErrorSource::Borsh(err),
        }
    }
}

// Automatic From implementations
impl From<bincode::Error> for ZkVmError {
    fn from(err: bincode::Error) -> Self {
        ZkVmError::SerializationError {
            source: SerializationErrorSource::Bincode(err),
        }
    }
}
