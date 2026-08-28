use rusqlite::Error as SqliteError;
use std::io;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("io error at {path:?}: {source}")]
    Io {
        path: Option<PathBuf>,
        #[source]
        source: io::Error,
    },
    #[error("sqlite error: {0}")]
    Sqlite(#[from] SqliteError),
    #[error("project not found: {0}")]
    NotFound(String),
    #[error("path is outside authorized scan roots: {0}")]
    Unauthorized(String),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Io { path: None, source }
    }
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
