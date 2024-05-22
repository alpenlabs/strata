use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid sync event index {0}")]
    MissingSyncEvent(u64),

    #[error("db: {0}")]
    Db(#[from] alpen_vertex_db::errors::DbError),

    #[error("not yet implemented")]
    Unimplemented,

    #[error("{0}")]
    Other(String),
}
