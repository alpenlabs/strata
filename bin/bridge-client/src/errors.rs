use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum InitError {
    #[error("Invalid operation mode, expected: operator(op) or challenger(ch), got: {0}")]
    InvalidMode(String),
}
