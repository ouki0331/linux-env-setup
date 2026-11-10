//! Schema validation for imported AI catalog JSON (`catalog-v0.1.json`).
//!
//! Guidelines from spec:
//! - Validates schema_version == "0.1"
//! - Rejects unknown or malformed top-level fields
//! - Validates command_id must match known inventory candidates
//! - Enforces bounds on field lengths and example counts
//! - Collects all errors and returns Error::Validation(Vec<String>)

use crate::catalog::{CatalogEntry, Example, Provider, ReviewState};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogImportPayload {
    pub schema_version: String,
    pub entries: Vec<CatalogImportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogImportEntry {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
}

/// Validate raw JSON string of enriched catalog and convert to CatalogEntries.
pub fn validate_and_parse_catalog_json(
    raw_json: &str,
    allowed_command_ids: Option<&HashSet<String>>,
) -> Result<Vec<CatalogEntry>> {
    let payload: CatalogImportPayload = serde_json::from_str(raw_json)
        .map_err(|e| Error::Validation(vec![format!("JSON root validation error: {e}")]))?;

    let mut errors = Vec::new();

    if payload.schema_version != "0.1" {
        errors.push(format!(
            "Unsupported schema_version '{}', expected '0.1'",
            payload.schema_version
        ));
    }

    if payload.entries.is_empty() {
        errors.push("Catalog import payload contains no entries".to_string());
    }

    let mut parsed_entries = Vec::new();
    let mut seen_ids = HashSet::new();

    for (idx, entry) in payload.entries.iter().enumerate() {
        let prefix = format!("entries[{idx}] ({})", entry.command_id);

        if entry.command_id.trim().is_empty() {
            errors.push(format!("{prefix}: command_id cannot be empty"));
        } else if !seen_ids.insert(entry.command_id.clone()) {
            errors.push(format!("{prefix}: duplicate command_id in batch"));
        }

        if entry.name.trim().is_empty() {
            errors.push(format!("{prefix}: name cannot be empty"));
        }

        if entry.name.len() > 100 {
            errors.push(format!("{prefix}: name exceeds 100 characters"));
        }

        if entry.summary.len() > 500 {
            errors.push(format!("{prefix}: summary exceeds 500 characters"));
        }

        if entry.syntax.len() > 300 {
            errors.push(format!("{prefix}: syntax exceeds 300 characters"));
        }

        if entry.examples.len() > 15 {
            errors.push(format!(
                "{prefix}: maximum 15 examples permitted, found {}",
                entry.examples.len()
            ));
        }

        for (e_idx, ex) in entry.examples.iter().enumerate() {
            if ex.command.trim().is_empty() {
                errors.push(format!(
                    "{prefix}.examples[{e_idx}]: command cannot be empty"
                ));
            }
        }

        // Validate command_id existence in inventory if context is provided
        if let Some(allowed) = allowed_command_ids {
            if !allowed.contains(&entry.command_id) {
                errors.push(format!(
                    "{prefix}: command_id does not exist in local inventory (AI hallucination guard)",
                ));
            }
        }

        let mut catalog_entry = CatalogEntry::new(&entry.command_id, &entry.name, &entry.kind);
        catalog_entry.summary = entry.summary.clone();
        catalog_entry.syntax = entry.syntax.clone();
        catalog_entry.examples = entry.examples.clone();
        catalog_entry.notes = entry.notes.clone();
        catalog_entry.tags = entry.tags.clone();
        catalog_entry.provider = entry.provider.clone();
        catalog_entry.documented_against_version =
            entry.provider.as_ref().and_then(|p| p.version.clone());
        catalog_entry.review_state = ReviewState::Documented;

        parsed_entries.push(catalog_entry);
    }

    if !errors.is_empty() {
        return Err(Error::Validation(errors));
    }

    Ok(parsed_entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_catalog_payload() {
        let json = r#"{
            "schema_version": "0.1",
            "entries": [
                {
                    "command_id": "cargo:ripgrep:rg",
                    "name": "rg",
                    "kind": "rust_binary",
                    "summary": "Recursively searches directories for a regex pattern.",
                    "syntax": "rg [OPTIONS] PATTERN [PATH ...]",
                    "examples": [
                        { "description": "Search for TODO", "command": "rg TODO" }
                    ],
                    "tags": ["search", "text"]
                }
            ]
        }"#;

        let entries = validate_and_parse_catalog_json(json, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "rg");
        assert_eq!(entries[0].examples.len(), 1);
    }

    #[test]
    fn test_invalid_schema_version_and_hallucination_guard() {
        let json = r#"{
            "schema_version": "0.2",
            "entries": [
                {
                    "command_id": "fake:command:id",
                    "name": "fake",
                    "kind": "binary"
                }
            ]
        }"#;

        let allowed = HashSet::from(["real:command:id".to_string()]);
        let res = validate_and_parse_catalog_json(json, Some(&allowed));
        assert!(res.is_err());
        let err = res.unwrap_err();
        if let Error::Validation(msgs) = err {
            assert!(msgs
                .iter()
                .any(|m| m.contains("Unsupported schema_version")));
            assert!(msgs
                .iter()
                .any(|m| m.contains("does not exist in local inventory")));
        } else {
            panic!("Expected Validation error");
        }
    }
}
