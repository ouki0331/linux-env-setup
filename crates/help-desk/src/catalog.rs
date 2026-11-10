//! Catalog data models and queries.
//!
//! Guidelines from spec:
//! - Single-language UTF-8 strings (no *_ja or *_en suffixes)
//! - Clean schema with review states:
//!   - `documented`: verified and documented
//!   - `needs_review`: provider package upgraded/downgraded or commands changed
//!   - `stale_pending_confirmation`: command vanished from latest snapshot, pending user confirmation
//!   - `unavailable`: confirmed uninstalled/archived

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Documented,
    NeedsReview,
    StalePendingConfirmation,
    Unavailable,
}

impl ReviewState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewState::Documented => "documented",
            ReviewState::NeedsReview => "needs_review",
            ReviewState::StalePendingConfirmation => "stale_pending_confirmation",
            ReviewState::Unavailable => "unavailable",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "needs_review" => ReviewState::NeedsReview,
            "stale_pending_confirmation" => ReviewState::StalePendingConfirmation,
            "unavailable" => ReviewState::Unavailable,
            _ => ReviewState::Documented,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    pub description: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    pub ecosystem: String,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_method: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub command_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub syntax: String,
    #[serde(default)]
    pub examples: Vec<Example>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documented_against_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceEvidence>,
    pub updated_at: String,
    pub review_state: ReviewState,
}

impl CatalogEntry {
    pub fn new(
        command_id: impl Into<String>,
        name: impl Into<String>,
        kind: impl Into<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            name: name.into(),
            kind: kind.into(),
            summary: String::new(),
            syntax: String::new(),
            examples: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            provider: None,
            documented_against_version: None,
            source: None,
            updated_at: crate::util::now_iso(),
            review_state: ReviewState::Documented,
        }
    }
}
