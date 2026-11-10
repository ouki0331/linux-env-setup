//! hdesk: a personal, single-language CLI help desk.
//!
//! The binary in `main.rs` is a thin wrapper around [`cli::run`]. Every
//! responsibility lives in its own module so it can be unit tested without
//! touching the real home directory, shell configuration, or package managers.

pub mod aggregate;
pub mod aiexport;
pub mod catalog;
pub mod cli;
pub mod config;
pub mod diff;
pub mod error;
pub mod exec;
pub mod inventory;
pub mod paths;
pub mod redact;
pub mod release;
pub mod schema;
pub mod snapshot;
pub mod sources;
pub mod storage;
pub mod util;
pub mod version;
