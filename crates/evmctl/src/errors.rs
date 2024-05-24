use thiserror::Error;

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("not yet implemented")]
    Unimplemented,

    #[error("{0}")]
    Other(String),
}
