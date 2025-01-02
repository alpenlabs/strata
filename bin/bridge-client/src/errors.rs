use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum InitError {
    #[error("Invalid operation mode, expected: operator(op) or challenger(ch), got: {0}")]
    InvalidMode(String),
}

#[derive(Debug, Clone, Error)]
pub enum PollDutyError {
    #[error("Rpc client: {0}")]
    RpcError(String),

    #[error("fetching websocket client from pool failed")]
    WsPool,
}
