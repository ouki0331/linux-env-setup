//! Generation of minimized, redacted JSON payloads for external AI models.

use crate::catalog::CatalogEntry;
use crate::diff::{diff_inventories, ChangeKind};
use crate::error::{Error, Result};
use crate::inventory::Inventory;
use crate::redact::{redact_alias_for_ai, redact_function_for_ai};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExportPayload {
    pub schema_version: String,
    pub generated_at: String,
    pub instructions: String,
    pub commands: Vec<AiCommandItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCommandItem {
    pub command_id: String,
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_ecosystem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_summary: Option<String>,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChangesExportPayload {
    pub schema_version: String,
    pub generated_at: String,
    pub from_snapshot: String,
    pub to_snapshot: String,
    pub instructions: String,
    pub updates: Vec<AiChangeItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChangeItem {
    pub command_id: String,
    pub name: String,
    pub kind: String,
    pub change_type: String,
    pub change_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_catalog_entry: Option<CatalogEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_summary: Option<String>,
}

/// Generate full AI input payload from an inventory snapshot.
pub fn generate_ai_input(inv: &Inventory, home: &Path) -> AiExportPayload {
    let mut warnings = Vec::new();
    let mut items = Vec::new();

    for cmd in &inv.commands {
        let def_summary = match cmd.kind.as_str() {
            "alias" => cmd
                .definition
                .as_deref()
                .map(|d| redact_alias_for_ai(d, home, &mut warnings)),
            "function" => cmd
                .definition
                .as_deref()
                .map(|d| redact_function_for_ai(d, home, &mut warnings)),
            _ => None,
        };

        items.push(AiCommandItem {
            command_id: cmd.id.clone(),
            name: cmd.name.clone(),
            kind: cmd.kind.clone(),
            provider_ecosystem: cmd.provider.as_ref().map(|p| p.ecosystem.clone()),
            provider_package: cmd.provider.as_ref().map(|p| p.package.clone()),
            provider_version: cmd.provider.as_ref().and_then(|p| p.version.clone()),
            definition_summary: def_summary,
            confidence: cmd.confidence.clone(),
        });
    }

    AiExportPayload {
        schema_version: "0.1".to_string(),
        generated_at: crate::util::now_iso(),
        instructions: "Please enrich each command with a concise summary (1-2 sentences), syntax, tags, and 1-3 everyday practical examples. Return standard catalog-v0.1.json format.".to_string(),
        commands: items,
        warnings,
    }
}

/// Generate changes-only AI export payload comparing two inventories.
pub fn generate_changes_only_export(
    from_inv: &Inventory,
    to_inv: &Inventory,
    current_catalog: &[CatalogEntry],
    home: &Path,
) -> Result<AiChangesExportPayload> {
    let diff = diff_inventories(from_inv, to_inv);
    let mut warnings = Vec::new();
    let mut updates = Vec::new();

    for cd in diff.commands {
        // Only include additions or changes that need review (exclude Removed/Unchanged)
        if matches!(cd.change, ChangeKind::Unchanged | ChangeKind::Removed) {
            continue;
        }

        let cand = cd
            .new_candidate
            .as_ref()
            .ok_or_else(|| Error::Other("Missing new candidate in change diff".into()))?;

        let def_summary = match cand.kind.as_str() {
            "alias" => cand
                .definition
                .as_deref()
                .map(|d| redact_alias_for_ai(d, home, &mut warnings)),
            "function" => cand
                .definition
                .as_deref()
                .map(|d| redact_function_for_ai(d, home, &mut warnings)),
            _ => None,
        };

        let existing_entry = current_catalog
            .iter()
            .find(|e| e.command_id == cd.command_id)
            .cloned();

        let old_v = cd
            .old_candidate
            .as_ref()
            .and_then(|c| c.provider.as_ref())
            .and_then(|p| p.version.clone());
        let new_v = cand.provider.as_ref().and_then(|p| p.version.clone());

        updates.push(AiChangeItem {
            command_id: cd.command_id,
            name: cd.name,
            kind: cand.kind.clone(),
            change_type: cd.change.as_str().to_string(),
            change_reason: cd.reason,
            old_version: old_v,
            new_version: new_v,
            current_catalog_entry: existing_entry,
            definition_summary: def_summary,
        });
    }

    Ok(AiChangesExportPayload {
        schema_version: "0.1".to_string(),
        generated_at: crate::util::now_iso(),
        from_snapshot: from_inv.snapshot_id.clone(),
        to_snapshot: to_inv.snapshot_id.clone(),
        instructions: "For each changed or added command, review if syntax or features changed. If evidence is insufficient to verify changes, retain existing summary/examples. Return catalog-v0.1.json containing only these updated items.".to_string(),
        updates,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::CommandCandidate;

    #[test]
    fn test_generate_ai_input_redacts_credentials() {
        let mut inv = Inventory::new("snap-1".into(), true);
        inv.commands.push(CommandCandidate {
            id: "zsh:alias:my_git".into(),
            name: "my_git".into(),
            kind: "alias".into(),
            provider: None,
            executable_path: None,
            definition: Some(
                "git push https://bot:ghp_1234567890abcdef1234567890abcdef1234@github.com".into(),
            ),
            source_files: vec![],
            effective: true,
            confidence: "high".into(),
            discovery_method: "zsh".into(),
            warnings: vec![],
        });

        let home = Path::new("/home/user");
        let payload = generate_ai_input(&inv, home);

        assert_eq!(payload.commands.len(), 1);
        let summary = payload.commands[0].definition_summary.as_ref().unwrap();
        assert!(!summary.contains("ghp_"));
        assert!(summary.contains("[REDACTED_SECRET]"));
        assert!(!payload.warnings.is_empty());
    }
}
