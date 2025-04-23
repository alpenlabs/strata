use std::{
    error::Error,
    fmt::{self, Debug, Display},
};

pub(crate) type BoxedInner = dyn std::error::Error + Send + Sync;
pub(crate) type BoxedErr = Box<BoxedInner>;

/// Boxed error with context
#[derive(Debug)]
pub struct BoxedErrWithContext {
    pub context: &'static str,
    pub source: BoxedErr,
}

impl BoxedErrWithContext {
    pub fn new<E>(context: &'static str, err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            context,
            source: Box::new(err),
        }
    }

    pub fn from_boxed(context: &'static str, err: BoxedErr) -> Self {
        Self {
            context,
            source: err,
        }
    }

    pub fn from_debug<E>(context: &'static str, err: E) -> Self
    where
        E: std::fmt::Debug + Send + Sync + 'static,
    {
        #[derive(Debug)]
        struct DebugWrapper<T: std::fmt::Debug + Send + Sync + 'static>(T);

        impl<T: std::fmt::Debug + Send + Sync + 'static> std::fmt::Display for DebugWrapper<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        impl<T: std::fmt::Debug + Send + Sync + 'static> std::error::Error for DebugWrapper<T> {}

        let wrapped = DebugWrapper(err);
        Self::from_boxed(context, Box::new(wrapped))
    }
}

impl Display for BoxedErrWithContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.source)
    }
}

impl Error for BoxedErrWithContext {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

macro_rules! simple_err {
    ($(#[$meta:meta])* $name:ident, $msg:literal) => {
        #[derive(Debug)]
        $(#[$meta])*
        pub struct $name;

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $msg)
            }
        }

        impl std::error::Error for $name {}
    };

    ($(#[$meta:meta])* $name:ident, $msg:literal, with_input) => {
        #[derive(Debug)]
        $(#[$meta])*
        pub struct $name(pub String);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!($msg, ": {}"), self.0)
            }
        }

        impl std::error::Error for $name {}
    };
}

macro_rules! boxed_err_with_context {
    ($name:ident) => {
        impl $name {
            pub fn new<E>(context: &'static str, err: E) -> Self
            where
                E: std::error::Error + Send + Sync + 'static,
            {
                Self(BoxedErrWithContext::new(context, err))
            }

            pub fn from_boxed(
                context: &'static str,
                err: Box<dyn std::error::Error + Send + Sync>,
            ) -> Self {
                Self(BoxedErrWithContext::from_boxed(context, err))
            }

            pub fn from_debug<E>(context: &'static str, err: E) -> Self
            where
                E: std::fmt::Debug + Send + Sync + 'static,
            {
                Self(BoxedErrWithContext::from_debug(context, err))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.source()
            }
        }
    };
}

#[macro_export]
macro_rules! handle_or_exit {
    ($expr:expr) => {
        if let Err(e) = $expr {
            eprintln!("Error: {:?}", e);
            std::process::exit(1);
        }
    };
}

/// This indicates runtime failure in the underlying platform storage system. The details of the
/// failure can be retrieved from the attached platform error.
#[derive(Debug)]
#[allow(unused)]
pub struct PlatformFailure(BoxedErr);

impl PlatformFailure {
    pub fn new<E>(e: E) -> Self
    where
        E: Into<BoxedErr>,
    {
        Self(e.into())
    }
}

/// This indicates that the underlying secure storage holding saved items could not be accessed.
/// Typically this is because of access rules in the platform; for example, it might be that the
/// credential store is locked. The underlying platform error will typically give the reason.
#[derive(Debug)]
#[allow(unused)]
pub struct NoStorageAccess(BoxedErr);

impl NoStorageAccess {
    pub fn new<E>(e: E) -> Self
    where
        E: Into<BoxedErr>,
    {
        Self(e.into())
    }
}
simple_err!(
    /// Invalid `faucet_endpoint` in config file
    InvalidFaucetEndpoint,
    "Invalid faucet endpoint. Please check your config file.",
    with_input
);

/// Errors related to claiming from faucet
#[derive(Debug)]
pub struct FaucetClaimError(BoxedErrWithContext);
boxed_err_with_context!(FaucetClaimError);

/// Errors related to signet chain
#[derive(Debug)]
pub struct SignetChainError(BoxedErrWithContext);
boxed_err_with_context!(SignetChainError);

/// Errors related to signet wallet
#[derive(Debug)]
pub struct SignetWalletError(BoxedErrWithContext);
boxed_err_with_context!(SignetWalletError);

/// Errors related to strata wallet
#[derive(Debug)]
pub struct StrataWalletError(BoxedErrWithContext);
boxed_err_with_context!(StrataWalletError);

/// Errors related to signet transactions
#[derive(Debug)]
pub struct SignetTxError(BoxedErrWithContext);
boxed_err_with_context!(SignetTxError);

/// Errors related to strata transactions
#[derive(Debug)]
pub struct StrataTxError(BoxedErrWithContext);
boxed_err_with_context!(StrataTxError);

/// Errors related to recovery
#[derive(Debug)]
pub struct DescriptorRecoveryError(BoxedErrWithContext);
boxed_err_with_context!(DescriptorRecoveryError);

simple_err!(
    /// Invalid `strata_endpoint` in config file
    InvalidStrataEndpoint,
    "Invalid strata endpoint. Please check your config file.",
    with_input
);

simple_err!(
    /// Invalid `signet` address
    InvalidSignetAddress,
    "Invalid signet address. Must be a valid Bitcoin address.",
    with_input
);

simple_err!(
    /// Invalid `strata` address
    InvalidStrataAddress,
    "Invalid Strata address. Must be a valid EVM-compatible address.",
    with_input
);

simple_err!(
    /// No address provided for `signet` or `strata` network
    MissingTargetAddress,
    "Missing address. Either a signet or Strata address must be provided."
);

simple_err!(
    /// Unsupported language for the mnemonic
    UnsupportedLanguage,
    "Unsupported language. Use --help to list supported languages.",
    with_input
);

simple_err!(
    /// Unsupported network for the wallet, must be `signet` or `strata`
    UnsupportedNetwork,
    "Unsupported network. Use 'signet' or 'strata'.",
    with_input
);

/// Wrong network for wallet address, e.g. `signet` address used in `strata` network
#[derive(Debug)]
pub struct WrongNetwork {
    pub address: String,
    pub network: String,
}

impl std::fmt::Display for WrongNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Address '{}' is not for the expected network '{}'",
            self.address, self.network
        )
    }
}

impl std::error::Error for WrongNetwork {}
