use thiserror::Error;

/// Strata CLI error types
#[derive(Debug, Error)]
pub enum CliError {
    /// Errors related to user input
    #[error("Invalid input: {0}")]
    UserInput(#[from] UserInputError),

    /// Internal errors while handling user requests
    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

/// Errors related to user input
///
/// These are errors that can be avoided if the user provides expected input.
#[derive(Debug, thiserror::Error)]
pub enum UserInputError {
    /// Unsupported language for printing the mnemonic
    #[error("unsupported language. Use --help to see available options.")]
    UnsupportedLanguage,

    /// Unsupported network for the wallet, must be `signet` or `strata`
    #[error("unsupported network. Use --help to see available options.")]
    UnsupportedNetwork,

    /// Wrong network for wallet address, e.g. `signet` address used in `strata` network
    #[error("wrong network for address")]
    WrongNetwork,

    /// Invalid `signet` address
    #[error("invalid signet address. Must be a valid Bitcoin address.")]
    InvalidSignetAddress,

    /// Invalid `strata` address
    #[error("invalid Strata address. Must be a valid EVM address.")]
    InvalidStrataAddress,

    /// No address provided for `signet` or `strata` network
    #[error("either signet or Strata address must be provided")]
    MissingTargetAddress,
}
