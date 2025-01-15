use pyo3::{exceptions::PyTypeError, prelude::*};

/// Error types for the functional tests.
#[derive(Debug, Clone)]
pub(crate) enum Error {
    /// Could not create a wallet.
    Wallet,

    /// Invalid Execution Layer address.
    ElAddress,

    /// Invalid XOnlyPublicKey.
    XOnlyPublicKey,

    /// Invalid PublicKey
    PublicKey,

    /// Invalid Outpoint.
    OutPoint,

    /// Not a Taproot address.
    NotTaprootAddress,

    /// Invalid Bitcoin address.
    BitcoinAddress,

    /// `OP_RETURN` bigger than 80 bytes.
    OpReturnTooLong,

    /// Could not create a BitcoinD RPC client.
    RpcClient,

    /// Error with BitcoinD response.
    BitcoinD,
}

/// Converts an `Error` into a `PyErr` to be raised in Python.
impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        match err {
            Error::Wallet => PyErr::new::<PyTypeError, _>("Could not create wallet".to_owned()),
            Error::ElAddress => {
                PyErr::new::<PyTypeError, _>("Invalid Execution Layer address".to_owned())
            }
            Error::XOnlyPublicKey => {
                PyErr::new::<PyTypeError, _>("Invalid X-only public key".to_owned())
            }
            Error::PublicKey => PyErr::new::<PyTypeError, _>("Invalid public key".to_owned()),
            Error::OutPoint => PyErr::new::<PyTypeError, _>("Invalid outpoint".to_owned()),
            Error::NotTaprootAddress => {
                PyErr::new::<PyTypeError, _>("Not a P2TR address".to_owned())
            }
            Error::BitcoinAddress => {
                PyErr::new::<PyTypeError, _>("Not a valid bitcoin address".to_owned())
            }
            Error::OpReturnTooLong => {
                PyErr::new::<PyTypeError, _>("OP_RETURN bigger than 80 bytes".to_owned())
            }
            Error::RpcClient => {
                PyErr::new::<PyTypeError, _>("Could not create RPC client".to_owned())
            }
            Error::BitcoinD => PyErr::new::<PyTypeError, _>("Invalid BitcoinD response".to_owned()),
        }
    }
}
