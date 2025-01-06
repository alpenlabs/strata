use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum InitError {
    #[error("Invalid operation mode, expected: operator(op) or challenger(ch), got: {0}")]
    InvalidMode(String),
}

/// Errors that can occur while polling for duties.
#[derive(Debug, Clone, Error)]
pub enum PollDutyError {
    /// An error occurred with the RPC client.
    #[error("RPC client: {0}")]
    Rpc(String),

    /// Failed to fetch a WebSocket client from the pool.
    #[error("fetching WebSocket client from pool failed")]
    WsPool,
}

/// Errors related to task management.
#[derive(Debug, Clone, Error)]
pub enum TaskManagerError {
    /// Maximum number of retries has been exceeded.
    #[error("Maximum retries exceeded. Num retries {0}")]
    MaxRetry(u16),
}
