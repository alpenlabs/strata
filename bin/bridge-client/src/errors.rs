use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum InitError {
    #[error("Invalid operation mode, expected: operator(op) or challenger(ch), got: {0}")]
    InvalidMode(String),
}

#[derive(Debug, Clone, Error)]
pub enum PollDutyError {
    #[error("RPC client: {0}")]
    Rpc(String),

    #[error("fetching WebSocket client from pool failed")]
    WsPool,
}

#[derive(Debug, Clone, Error)]
pub enum TaskManagerError {
    #[error("Polling Duty Failed: {0}")]
    Poll(#[from] PollDutyError),

    #[error("Maximum retries exceeded. Num retries {0}")]
    MaxRetry(u16),
}
