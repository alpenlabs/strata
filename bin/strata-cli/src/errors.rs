use thiserror::Error;

/// Errors related to user input
///
/// These are errors that can be avoided if the user provides expected input.
#[derive(Debug, Error)]
pub enum UserInputError {
    /// Invalid `signet` address
    #[error("Invalid faucet url. Ensure faucet URL is correct in config file.")]
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
    #[error("Failed to add descriptor: {0}")]
    AddDescriptor(String),

    #[error("Failed to broadcast signet transaction: {0}")]
    BroadcastSignetTxn(String),

    #[error("Failed to broadcast strata transaction: {0}")]
    BroadcastStrataTxn(String),

    #[error("Failed to build signet transaction: {0}")]
    BuildSignetTxn(String),

    #[error("Failed to claim from faucet: {0}")]
    ClaimFromFaucet(String),

    #[error("Failed to convert to wallet descriptor: {0}")]
    ConvertToWalletDescriptor(String),

    #[error("Failed to create recovery wallet: {0}")]
    CreateRecoveryWallet(String),

    #[error("Failed to create temporary wallet: {0}")]
    CreateTemporaryWallet(String),

    #[error("Failed to delete seed: {0}")]
    DeleteSeed(String),

    #[error("Failed to delete data directory: {0}")]
    DeleteDataDirectory(String),

    #[error("Failed to encrypt seed: {0}")]
    EncryptSeed(String),

    #[error("Failed to estimate strata gas: {0}")]
    EstimateStrataGas(String),

    #[error("Failed to extract transaction: {0}")]
    ExtractSignetTxn(String),

    #[error("Failed to extract P2TR public key: {0}")]
    ExtractP2trPubkey(String),

    #[error("Failed to fetch PoW challenge from faucet: {0}")]
    FectFaucetPowChallenge(String),

    #[error("Failed to fetch strata balance: {0}")]
    FetchStrataBalance(String),

    #[error("Failed to fetch strata gas price: {0}")]
    FetchStrataGasPrice(String),

    #[error("Failed to generate bridge-in descriptor: {0}")]
    GenerateBridgeInDescriptor(String),

    #[error("Failed to get signet chain tip: {0}")]
    GetSignetChainTip(String),

    #[error("Failed to load signet wallet: {0}")]
    LoadSignetWallet(String),

    #[error("Failed to load strata wallet: {0}")]
    LoadStrataWallet(String),

    #[error("Failed to open descriptor database: {0}")]
    OpenDescriptorDatabase(String),

    #[error("Failed to parse faucet response: {0}")]
    ParseFaucetResponse(String),

    #[error("Failed to save encrypted seed: {0}")]
    PersistEncryptedSeed(String),

    #[error("Failed to persist signet wallet: {0}")]
    PersistSignetWallet(String),

    #[error("Failed to read reset confirmation: {0}")]
    ReadConfirmation(String),

    #[error("Failed to read descriptors: {0}")]
    ReadDescriptors(String),

    #[error("Failed to read password: {0}")]
    ReadPassword(String),

    #[error("Failed to remove old descriptor: {0}")]
    RemoveDescriptor(String),

    #[error("Failed to scan signet wallet: {0}")]
    ScanSignetWallet(String),

    #[error("Failed to sign signet transaction: {0}")]
    SignSignetTxn(String),

    #[error("Failed to sync signet wallet: {0}")]
    SyncSignetWallet(String),

    #[error("Failed to sync recovery wallet: {0}")]
    SyncRecoveryWallet(String),

    #[error("Failed to sync signet wallet: {0}")]
    UnexpectedFaucetResponse(String),
}
