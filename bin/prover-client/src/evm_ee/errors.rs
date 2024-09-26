use thiserror::Error;

// Define custom error type
#[derive(Error, Debug)]
pub enum ELProvingTaskError {
    #[error("Failed to fetch EL block prover input for block number {block_num}: {source}")]
    FetchElBlockProverInputError {
        block_num: u64,
        source: anyhow::Error,
    },

    #[error("Failed to serialize the EL block prover input")]
    SerializationError(#[from] bincode::Error),
}
