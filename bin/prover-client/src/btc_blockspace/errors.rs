use thiserror::Error;

// Define custom error type
#[derive(Error, Debug)]
pub enum BtcProvingTaskError {
    #[error("Failed to fetch BTC block prover input for block number {block_num}: {source}")]
    FetchBtcBlockProverInputError {
        block_num: u64,
        source: anyhow::Error,
    },

    #[error("Failed to serialize the BTC block prover input")]
    SerializationError(#[from] bincode::Error),
}
