//! Error type and exit-code mapping.
//!
//! Exit codes are part of the CLI contract:
//! - 0: success
//! - 1: runtime failure (I/O, database, external tool)
//! - 2: usage error (also used by clap for unknown options)
//! - 3: requested catalog entry or snapshot not found
//! - 4: validation failure (input JSON rejected; nothing was written)

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Usage(String),
    NotFound(String),
    Validation(Vec<String>),
    Io(String),
    Db(String),
    Json(String),
    Other(String),
}

impl Error {
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::Usage(_) => 2,
            Error::NotFound(_) => 3,
            Error::Validation(_) => 4,
            _ => 1,
        }
    }

    pub fn io(context: impl fmt::Display, err: std::io::Error) -> Self {
        Error::Io(format!("{context}: {err}"))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) => write!(f, "{m}"),
            Error::NotFound(m) => write!(f, "{m}"),
            Error::Validation(errors) => {
                write!(f, "validation failed with {} error(s):", errors.len())?;
                for e in errors {
                    write!(f, "\n  - {e}")?;
                }
                Ok(())
            }
            Error::Io(m) => write!(f, "I/O error: {m}"),
            Error::Db(m) => write!(f, "database error: {m}"),
            Error::Json(m) => write!(f, "JSON error: {m}"),
            Error::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
