//! Inventory data models representing local machine facts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: String,
    pub arch: String,
    pub shell: String,
}

impl HostInfo {
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            shell: "zsh".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFact {
    pub id: String,
    pub ecosystem: String,
    pub name: String,
    pub version: Option<String>,
    pub scope: String,
    #[serde(default)]
    pub commands: Vec<String>,
    pub discovery_method: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandProvider {
    pub ecosystem: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default = "default_scope")]
    pub scope: String,
}

fn default_scope() -> String {
    "global".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandCandidate {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<CommandProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
    pub effective: bool,
    pub confidence: String,
    pub discovery_method: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterReport {
    pub name: String,
    pub status: String, // "ok", "skipped", "failed"
    pub packages_found: usize,
    pub commands_found: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub schema_version: String,
    pub generated_at: String,
    pub snapshot_id: String,
    pub complete: bool,
    pub host: HostInfo,
    #[serde(default)]
    pub packages: Vec<PackageFact>,
    #[serde(default)]
    pub commands: Vec<CommandCandidate>,
    #[serde(default)]
    pub adapters: Vec<AdapterReport>,
}

impl Inventory {
    pub fn new(snapshot_id: String, complete: bool) -> Self {
        Self {
            schema_version: "0.1".to_string(),
            generated_at: crate::util::now_iso(),
            snapshot_id,
            complete,
            host: HostInfo::current(),
            packages: Vec::new(),
            commands: Vec::new(),
            adapters: Vec::new(),
        }
    }
}
