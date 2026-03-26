use thiserror::Error;

#[derive(Error, Debug)]
pub enum EdmsError {
    #[error("Unknown error occurred")]
    UnknownError,

    #[error("Invalid JSON format: {0}")]
    InvalidJson(String),

    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("SQLite file not found")]
    SqliteFileNotFound,

    #[error("SQLite file locked")]
    SqliteFileLocked,

    #[error("SQLite consistency check failed")]
    SqliteConsistencyFailed,

    #[error("SQLite initialization failed")]
    SqliteInitializationFailed,

    #[error("Query execution failed (code: {0})")]
    QueryExecutionFailed(i32),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type EdmsResult<T> = Result<T, EdmsError>;

pub fn result_to_code<T>(result: EdmsResult<T>) -> i32 {
    match result {
        Ok(_) => 0,
        Err(EdmsError::QueryExecutionFailed(code)) => code,
        Err(_) => -1,
    }
}
