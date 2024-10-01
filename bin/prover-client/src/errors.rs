use thiserror::Error;

// Define custom error type
#[derive(Error, Debug)]
pub enum ProvingTaskError {
    #[error(
        "Failed to fetch {task_type} block prover input for block number {block_num}: {source}"
    )]
    FetchInputError {
        block_num: u64,
        task_type: BlockType,
        source: anyhow::Error,
    },

    #[error("Failed to serialize the EL block prover input")]
    SerializationError(#[from] bincode::Error),
}

// Define BlockType enum to represent EL and CL
#[derive(Debug, Clone, Copy)]
pub enum BlockType {
    Btc,
    EL,
    CL,
}

impl std::fmt::Display for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let block_type_str = match self {
            BlockType::Btc => "BTC",
            BlockType::EL => "EL",
            BlockType::CL => "CL",
        };
        write!(f, "{}", block_type_str)
    }
}
