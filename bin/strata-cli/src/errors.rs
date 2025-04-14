use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Invalid input: {0}")]
    UserInput(#[from] UserInputError),

    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UserInputError {
    #[error("unsupported language. Use --help to see available options.")]
    UnsupportedLanguage,

    #[error("unsupported network. Use --help to see available options.")]
    UnsupportedNetwork,

    #[error("wrong network for address")]
    WrongNetwork,

    #[error("invalid signet address. Must be a valid Bitcoin address.")]
    InvalidSignetAddress,

    #[error("invalid Strata address. Must be a valid EVM address.")]
    InvalidStrataAddress,

    #[error("either signet or Strata address must be provided")]
    MissingTargetAddress,
}

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("wallet not initialized")]
    NotInitialized,

    #[error("failed to load wallet")]
    LoadFailed,

    #[error("failed to scan wallet")]
    ScanFailed,

    #[error("failed to sync wallet")]
    SyncFailed,

    #[error("failed to save wallet state")]
    PersistFailed,

    #[error("invalid wallet descriptor")]
    InvalidWalletDescriptor,
}
