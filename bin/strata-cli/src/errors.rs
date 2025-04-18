use thiserror::Error;

/// Errors from CLI commands
#[derive(Debug, Error)]
pub enum CliError {
    /// Errors the user can take action to address
    #[error("Invalid input: {0}")]
    UserInputError(#[from] UserInputError),

    /// Errors that occur while handling user requests
    #[error("Internal error: {0}")]
    InternalError(#[from] InternalError),
}

/// Errors related to user input
///
/// The user can address these providing expected input.
#[derive(Debug, Error)]
pub enum UserInputError {
    /// Invalid `signet` address
    #[error("Invalid faucet URL. Ensure faucet URL is correct in config file.")]
    InvalidFaucetUrl,

    /// Invalid `signet` address
    #[error("Invalid signet address. Must be a valid Bitcoin address.")]
    InvalidSignetAddress,

    /// Invalid `strata` address
    #[error("Invalid Strata address. Must be a valid EVM address.")]
    InvalidStrataAddress,

    /// No address provided for `signet` or `strata` network
    #[error("Missing target address. Either signet or Strata address must be provided")]
    MissingTargetAddress,

    /// Unsupported language for printing the mnemonic
    #[error("Unsupported language. Use --help to see available options.")]
    UnsupportedLanguage,

    /// Unsupported network for the wallet, must be `signet` or `strata`
    #[error("Unsupported network. Use --help to see available options.")]
    UnsupportedNetwork,

    /// Wrong network for wallet address, e.g. `signet` address used in `strata` network
    #[error("Wrong network for address")]
    WrongNetwork,
}

/// Internal errors that can occur while handling user requests
#[derive(Debug, Error)]
pub enum InternalError {
    /// Failed to open descriptor database
    #[error("Failed to open descriptor database. {0}")]
    OpenDescriptorDatabase(String),

    /// Failed to read descriptors from descriptor database
    #[error("Failed to read descriptors. {0}")]
    ReadDescriptors(String),

    /// Failed to add descriptor to descriptor database
    #[error("Failed to add descriptor. {0}")]
    AddDescriptor(String),

    /// Failed to remove old descriptor
    #[error("Failed to remove old descriptor. {0}")]
    RemoveDescriptor(String),

    /// Failed to convert to wallet descriptor
    #[error("Failed to convert to wallet descriptor. {0}")]
    ConvertToWalletDescriptor(String),

    /// Failed to create recovery wallet
    #[error("Failed to create recovery wallet. {0}")]
    CreateRecoveryWallet(String),

    /// Failed to create temporary wallet
    #[error("Failed to create temporary wallet. {0}")]
    CreateTemporaryWallet(String),

    /// Failed to sync recovery wallet
    #[error("Failed to sync recovery wallet. {0}")]
    SyncRecoveryWallet(String),

    /// Provided address is not a taproot (P2TR) address
    #[error("Not a taproot (P2TR) address. {0}")]
    NotTaprootAddress(String),

    /// Failed to get signet chain tip
    #[error("Failed to get signet chain tip. {0}")]
    GetSignetChainTip(String),

    /// Failed to load signet wallet
    #[error("Failed to load signet wallet. {0}")]
    LoadSignetWallet(String),

    /// Failed to sync signet wallet
    #[error("Failed to sync signet wallet. {0}")]
    SyncSignetWallet(String),

    /// Failed to scan signet wallet
    #[error("Failed to scan signet wallet. {0}")]
    ScanSignetWallet(String),

    /// Failed to persist signet wallet
    #[error("Failed to persist signet wallet. {0}")]
    PersistSignetWallet(String),

    /// Failed to build signet transaction
    #[error("Failed to build signet transaction. {0}")]
    BuildSignetTxn(String),

    /// Failed to sign signet transaction
    #[error("Failed to sign signet transaction. {0}")]
    SignSignetTxn(String),

    // Failed to finalize signet transaction from PSBT
    #[error("Failed to finalize transaction from PSBT. {0}")]
    FinalizeSignetTxn(String),

    /// Failed to broadcast signet transaction
    #[error("Failed to broadcast signet transaction. {0}")]
    BroadcastSignetTxn(String),

    /// Failed to load strata wallet
    #[error("Failed to load strata wallet. {0}")]
    LoadStrataWallet(String),

    /// Failed to fetch gas price for strata transaction
    #[error("Failed to fetch strata gas price. {0}")]
    FetchStrataGasPrice(String),

    /// Failed to fetch strata wallet balance
    #[error("Failed to fetch strata balance. {0}")]
    FetchStrataBalance(String),

    /// Failed to get a gas estimate for strata transaction.
    /// Includes "insufficient funds".
    #[error("Failed to estimate strata gas. {0}")]
    EstimateStrataGas(String),

    /// Failed to broadcast strata transaction
    #[error("Failed to broadcast strata transaction. {0}")]
    BroadcastStrataTxn(String),

    /// Failed to encrypt seed
    #[error("Failed to encrypt seed. {0}")]
    EncryptSeed(String),

    /// Failed to persist encrypted seed
    #[error("Failed to save encrypted seed. {0}")]
    PersistEncryptedSeed(String),

    /// Failed to delete seed
    #[error("Failed to delete seed. {0}")]
    DeleteSeed(String),

    /// Failed to delete data directory
    #[error("Failed to delete data directory. {0}")]
    DeleteDataDirectory(String),

    /// Failed to read user-entered password
    #[error("Failed to read password. {0}")]
    ReadPassword(String),

    /// Failed to read user confirmation for reset
    #[error("Failed to read reset confirmation. {0}")]
    ReadConfirmation(String),

    /// Failed to fetch PoW challenge from faucet
    #[error("Failed to fetch PoW challenge from faucet. {0}")]
    FetchFaucetPowChallenge(String),

    /// Failed to claim from faucet
    #[error("Failed to claim from faucet. {0}")]
    ClaimFromFaucet(String),

    /// Failed to parse faucet JSON response
    #[error("Failed to parse faucet JSON response. {0}")]
    ParseFaucetResponse(String),

    /// Faucet response was not in expected format
    #[error("Unexpected response from faucet. {0}")]
    UnexpectedFaucetResponse(String),
}

/// Convnience function to go from [`InternalError`] to [`CliError`].
/// We are including the original error message.
pub fn internal_err<E, F>(f: F) -> impl FnOnce(E) -> CliError
where
    E: std::fmt::Debug,
    F: FnOnce(String) -> InternalError,
{
    move |e| CliError::InternalError(f(format!("{e:?}")))
}

/// Convnience function to go from [`UserInputError`] to [`CliError`].
pub fn user_err(variant: UserInputError) -> CliError {
    CliError::UserInputError(variant)
}
