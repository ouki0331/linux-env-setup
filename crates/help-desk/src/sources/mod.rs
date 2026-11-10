//! Discovery source adapters for personal and global CLI tools.

pub mod cargo;
pub mod composer;
pub mod npm;
pub mod path;
pub mod python;
pub mod zsh;

use crate::config::Config;
use crate::error::Result;
use crate::inventory::{CommandCandidate, PackageFact};
use crate::paths::Paths;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScanContext<'a> {
    pub paths: &'a Paths,
    pub config: &'a Config,
    pub allow_source: Option<PathBuf>,
    pub zsh_stdin: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterStatus {
    Ok,
    Skipped,
    Failed,
}

impl AdapterStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AdapterStatus::Ok => "ok",
            AdapterStatus::Skipped => "skipped",
            AdapterStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdapterResult {
    pub name: &'static str,
    pub status: AdapterStatus,
    pub packages: Vec<PackageFact>,
    pub commands: Vec<CommandCandidate>,
    pub error: Option<String>,
}

impl AdapterResult {
    pub fn ok(
        name: &'static str,
        packages: Vec<PackageFact>,
        commands: Vec<CommandCandidate>,
    ) -> Self {
        Self {
            name,
            status: AdapterStatus::Ok,
            packages,
            commands,
            error: None,
        }
    }

    pub fn skipped(name: &'static str, reason: impl Into<String>) -> Self {
        Self {
            name,
            status: AdapterStatus::Skipped,
            packages: Vec::new(),
            commands: Vec::new(),
            error: Some(reason.into()),
        }
    }

    pub fn failed(name: &'static str, err: impl Into<String>) -> Self {
        Self {
            name,
            status: AdapterStatus::Failed,
            packages: Vec::new(),
            commands: Vec::new(),
            error: Some(err.into()),
        }
    }
}

pub trait SourceAdapter {
    fn name(&self) -> &'static str;
    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult>;
}

pub fn all_adapters() -> Vec<Box<dyn SourceAdapter>> {
    vec![
        Box::new(zsh::ZshAdapter),
        Box::new(cargo::CargoAdapter),
        Box::new(npm::NpmAdapter),
        Box::new(python::PythonAdapter),
        Box::new(composer::ComposerAdapter),
        Box::new(path::PathAdapter),
    ]
}
