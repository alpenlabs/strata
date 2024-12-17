use thiserror::Error;

use crate::{ProofType, ZkVm};

/// A convenient alias for results in the ZkVM.
pub type ZkVmResult<T> = Result<T, ZkVmError>;

#[derive(Debug, Error)]
pub enum ZkVmError {
    #[error("Proof generation failed: {0}")]
    ProofGenerationError(String),

    #[error("Proof verification failed: {0}")]
    ProofVerificationError(String),

    #[error("Input validation failed: {0}")]
    InvalidInput(#[from] ZkVmInputError),

    #[error("ELF validation failed: {0}")]
    InvalidELF(String),

    #[error("Invalid Verification Key")]
    InvalidVerificationKey(#[from] ZkVmVerificationKeyError),

    #[error("Invalid proof receipt")]
    InvalidProofReceipt(#[from] ZkVmProofError),

    #[error("Output extraction failed")]
    OutputExtractionError {
        #[source]
        source: DataFormatError,
    },

    #[error("An unexpected error occurred: {0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum DataFormatError {
    #[error("error using bincode: {source}")]
    Bincode {
        #[source]
        source: bincode::Error,
    },

    #[error("error using borsh: {source}")]
    Borsh {
        #[source]
        source: borsh::io::Error,
    },

    #[error("error using serde: {0}")]
    Serde(String),

    #[error("error: {0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum ZkVmInputError {
    #[error("Input data format error")]
    DataFormat(#[source] DataFormatError),

    #[error("Input proof receipt error")]
    ProofReceipt(#[source] ZkVmProofError),

    #[error("Input verification key error")]
    VerificationKey(#[source] ZkVmVerificationKeyError),

    #[error("Input build error: {0}")]
    InputBuild(String),
}

#[derive(Debug, Error)]
pub enum ZkVmVerificationKeyError {
    #[error("Verification Key format error")]
    DataFormat(#[source] DataFormatError),

    #[error("Verification Key size error")]
    InvalidVerificationKeySize,
}

#[derive(Debug, Error)]
pub enum ZkVmProofError {
    #[error("Input data format error")]
    DataFormat(#[source] DataFormatError),

    #[error("Invalid ProofType: expected {0:?}")]
    InvalidProofType(ProofType),

    #[error("Invalid ZkVm: expected {0:?}, found {1:?}")]
    InvalidZkVm(ZkVm, ZkVm),
}

#[derive(Debug, Error)]
pub enum InvalidVerificationKeySource {
    #[error("Verification Key format error")]
    DataFormat(#[from] DataFormatError),
}

/// Implement automatic conversion for `bincode::Error` to `DataFormatError`
impl From<bincode::Error> for DataFormatError {
    fn from(err: bincode::Error) -> Self {
        DataFormatError::Bincode { source: err }
    }
}

/// Implement automatic conversion for `borsh::io::Error` to `DataFormatError`
impl From<borsh::io::Error> for DataFormatError {
    fn from(err: borsh::io::Error) -> Self {
        DataFormatError::Borsh { source: err }
    }
}

/// Implement automatic conversion for `bincode::Error` to `InvalidProofReceipt`
impl From<bincode::Error> for ZkVmProofError {
    fn from(err: bincode::Error) -> Self {
        let source = DataFormatError::Bincode { source: err };
        ZkVmProofError::DataFormat(source)
    }
}

/// Implement automatic conversion for `borsh::io::Error` to `InvalidProofReceiptSource`
impl From<borsh::io::Error> for ZkVmProofError {
    fn from(err: borsh::io::Error) -> Self {
        let source = DataFormatError::Borsh { source: err };
        ZkVmProofError::DataFormat(source)
    }
}

/// Implement automatic conversion for `bincode::Error` to `ZkVmInputError`
impl From<bincode::Error> for ZkVmInputError {
    fn from(err: bincode::Error) -> Self {
        let source = DataFormatError::Bincode { source: err };
        ZkVmInputError::DataFormat(source)
    }
}

/// Implement automatic conversion for `borsh::io::Error` to `ZkVmInputError`
impl From<borsh::io::Error> for ZkVmInputError {
    fn from(err: borsh::io::Error) -> Self {
        let source = DataFormatError::Borsh { source: err };
        ZkVmInputError::DataFormat(source)
    }
}
