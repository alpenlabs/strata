//! Error types for the RPC client.
use std::fmt;

use bitcoin::Network;
use serde::{Deserialize, Serialize};
use serde_json::Error as SerdeJsonError;
use thiserror::Error;

/// The error type for errors produced in this library.
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientError {
    /// Network error, retry might help
    #[error("Network: {0}")]
    Network(String),

    /// Missing username or password for the RPC server
    #[error("Missing username or password")]
    MissingUserPassword,

    /// RPC server returned an error
    ///
    /// # Note
    ///
    /// These errors are ABSOLUTELY UNDOCUMENTED.
    /// Check
    /// <https://github.com/bitcoin/bitcoin/blob/96b0a8f858ab24f3672360b8c830553b963de726/src/rpc/protocol.h#L24>
    /// and good luck!
    #[error("RPC server returned error '{1}' (code {0})")]
    Server(i32, String),

    #[error("Error parsing rpc response: {0}")]
    Parse(String),

    /// Error creating the RPC request, retry might help
    #[error("Could not create RPC Param")]
    Param(String),

    /// Body error, unlikely to be recoverable by retrying
    #[error("{0}")]
    Body(String),

    /// HTTP status error, not retryable
    #[error("Obtained failure status({0}): {1}")]
    Status(String, String),

    /// Error decoding the response, retry might not help
    #[error("Malformed Response: {0}")]
    MalformedResponse(String),

    /// Connection error, retry might help
    #[error("Could not connect: {0}")]
    Connection(String),

    /// Timeout error, retry might help
    #[error("Timeout")]
    Timeout,

    /// Redirect error, not retryable
    #[error("HttpRedirect: {0}")]
    HttpRedirect(String),

    /// Error building the request, unlikely to be recoverable
    #[error("Could not build request: {0}")]
    ReqBuilder(String),

    /// Maximum retries exceeded, not retryable
    #[error("Max retries {0} exceeded")]
    MaxRetriesExceeded(u8),

    /// General request error, retry might help
    #[error("Could not create request: {0}")]
    Request(String),

    /// Wrong network address
    #[error("Network address: {0}")]
    WrongNetworkAddress(Network),

    /// Server version is unexpected or incompatible
    #[error(transparent)]
    UnexpectedServerVersion(#[from] UnexpectedServerVersionError),

    /// Could not sign raw transaction
    #[error(transparent)]
    Sign(#[from] SignRawTransactionWithWalletError),

    /// Could not get a [`Xpriv`](bitcoin::bip32::Xpriv) from the wallet
    #[error("Could not get xpriv from wallet")]
    Xpriv,

    /// Unknown error, unlikely to be recoverable
    #[error("{0}")]
    Other(String),
}

impl From<SerdeJsonError> for ClientError {
    fn from(value: SerdeJsonError) -> Self {
        Self::Parse(format!("Could not parse {}", value))
    }
}

/// `bitcoind` RPC server error.
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitcoinRpcError {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for BitcoinRpcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RPC error {}: {}", self.code, self.message)
    }
}

impl From<BitcoinRpcError> for ClientError {
    fn from(value: BitcoinRpcError) -> Self {
        Self::Server(value.code, value.message)
    }
}

/// Error returned when signing a raw transaction with a wallet fails.
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignRawTransactionWithWalletError {
    /// The transaction ID.
    txid: String,
    /// The index of the input.
    vout: u32,
    /// The script signature.
    #[serde(rename = "scriptSig")]
    script_sig: String,
    /// The sequence number.
    sequence: u32,
    /// The error message.
    error: String,
}

impl fmt::Display for SignRawTransactionWithWalletError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "error signing raw transaction with wallet: {}",
            self.error
        )
    }
}

/// Error returned when RPC client expects a different version than bitcoind reports.
#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnexpectedServerVersionError {
    /// Version from server.
    pub got: usize,
    /// Expected server version.
    pub expected: Vec<usize>,
}

impl fmt::Display for UnexpectedServerVersionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut expected = String::new();
        for version in &self.expected {
            let v = format!(" {} ", version);
            expected.push_str(&v);
        }
        write!(
            f,
            "unexpected bitcoind version, got: {} expected one of: {}",
            self.got, expected
        )
    }
}
