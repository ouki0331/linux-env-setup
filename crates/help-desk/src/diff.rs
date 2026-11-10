//! Snapshot and inventory difference engine.
//!
//! Guidelines from spec:
//! - Diff states: unchanged, added, removed, upgraded, downgraded, version_changed,
//!   provider_changed, commands_changed, definition_changed.
//! - Incomplete snapshots or failed adapters MUST NOT falsely classify missing items as removed.
//! - Idempotent runs on the same environment MUST produce an empty diff.

use crate::inventory::{CommandCandidate, Inventory, PackageFact};
use crate::version::{compare_versions, VersionDiff};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Unchanged,
    Added,
    Removed,
    Upgraded,
    Downgraded,
    VersionChanged,
    ProviderChanged,
    CommandsChanged,
    DefinitionChanged,
}

impl ChangeKind {
    pub fn is_changed(&self) -> bool {
        !matches!(self, ChangeKind::Unchanged)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeKind::Unchanged => "unchanged",
            ChangeKind::Added => "added",
            ChangeKind::Removed => "removed",
            ChangeKind::Upgraded => "upgraded",
            ChangeKind::Downgraded => "downgraded",
            ChangeKind::VersionChanged => "version_changed",
            ChangeKind::ProviderChanged => "provider_changed",
            ChangeKind::CommandsChanged => "commands_changed",
            ChangeKind::DefinitionChanged => "definition_changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDiff {
    pub package_id: String,
    pub ecosystem: String,
    pub name: String,
    pub change: ChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_version: Option<String>,
    pub old_commands: Vec<String>,
    pub new_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDiff {
    pub command_id: String,
    pub name: String,
    pub change: ChangeKind,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_candidate: Option<CommandCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_candidate: Option<CommandCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
    pub is_empty: bool,
    pub packages: Vec<PackageDiff>,
    pub commands: Vec<CommandDiff>,
}

pub fn diff_inventories(from: &Inventory, to: &Inventory) -> SnapshotDiff {
    let mut package_diffs = Vec::new();
    let mut command_diffs = Vec::new();

    // Identify ecosystems that failed in the 'to' snapshot to prevent false removals
    let failed_ecosystems: HashSet<String> = to
        .adapters
        .iter()
        .filter(|a| a.status != "ok")
        .map(|a| a.name.to_lowercase())
        .collect();

    // 1. Package Diffs
    let from_pkgs: BTreeMap<&str, &PackageFact> =
        from.packages.iter().map(|p| (p.id.as_str(), p)).collect();
    let to_pkgs: BTreeMap<&str, &PackageFact> =
        to.packages.iter().map(|p| (p.id.as_str(), p)).collect();

    for (&id, &new_pkg) in &to_pkgs {
        if let Some(&old_pkg) = from_pkgs.get(id) {
            let v_diff = compare_versions(
                &new_pkg.ecosystem,
                old_pkg.version.as_deref(),
                new_pkg.version.as_deref(),
            );

            let change = match v_diff {
                VersionDiff::Upgraded => ChangeKind::Upgraded,
                VersionDiff::Downgraded => ChangeKind::Downgraded,
                VersionDiff::VersionChanged => ChangeKind::VersionChanged,
                VersionDiff::Unchanged => {
                    if old_pkg.commands != new_pkg.commands {
                        ChangeKind::CommandsChanged
                    } else {
                        ChangeKind::Unchanged
                    }
                }
            };

            if change.is_changed() {
                package_diffs.push(PackageDiff {
                    package_id: id.to_string(),
                    ecosystem: new_pkg.ecosystem.clone(),
                    name: new_pkg.name.clone(),
                    change,
                    old_version: old_pkg.version.clone(),
                    new_version: new_pkg.version.clone(),
                    old_commands: old_pkg.commands.clone(),
                    new_commands: new_pkg.commands.clone(),
                });
            }
        } else {
            // Added package
            package_diffs.push(PackageDiff {
                package_id: id.to_string(),
                ecosystem: new_pkg.ecosystem.clone(),
                name: new_pkg.name.clone(),
                change: ChangeKind::Added,
                old_version: None,
                new_version: new_pkg.version.clone(),
                old_commands: Vec::new(),
                new_commands: new_pkg.commands.clone(),
            });
        }
    }

    for (&id, &old_pkg) in &from_pkgs {
        if !to_pkgs.contains_key(id) {
            // Guard: If the ecosystem failed in 'to', do not mark as removed
            if failed_ecosystems.contains(&old_pkg.ecosystem.to_lowercase()) {
                continue;
            }
            package_diffs.push(PackageDiff {
                package_id: id.to_string(),
                ecosystem: old_pkg.ecosystem.clone(),
                name: old_pkg.name.clone(),
                change: ChangeKind::Removed,
                old_version: old_pkg.version.clone(),
                new_version: None,
                old_commands: old_pkg.commands.clone(),
                new_commands: Vec::new(),
            });
        }
    }

    // 2. Command Diffs
    let from_cmds: BTreeMap<&str, &CommandCandidate> =
        from.commands.iter().map(|c| (c.id.as_str(), c)).collect();
    let to_cmds: BTreeMap<&str, &CommandCandidate> =
        to.commands.iter().map(|c| (c.id.as_str(), c)).collect();

    for (&id, &new_cmd) in &to_cmds {
        if let Some(&old_cmd) = from_cmds.get(id) {
            // Check provider version change
            let old_ver = old_cmd.provider.as_ref().and_then(|p| p.version.as_deref());
            let new_ver = new_cmd.provider.as_ref().and_then(|p| p.version.as_deref());
            let eco = new_cmd
                .provider
                .as_ref()
                .map(|p| p.ecosystem.as_str())
                .unwrap_or("path");

            let v_diff = compare_versions(eco, old_ver, new_ver);

            let mut change = match v_diff {
                VersionDiff::Upgraded => ChangeKind::Upgraded,
                VersionDiff::Downgraded => ChangeKind::Downgraded,
                VersionDiff::VersionChanged => ChangeKind::VersionChanged,
                VersionDiff::Unchanged => ChangeKind::Unchanged,
            };

            // Check definition change (for alias/function)
            if old_cmd.definition != new_cmd.definition {
                change = ChangeKind::DefinitionChanged;
            }

            // Check provider change
            if old_cmd.provider != new_cmd.provider && change == ChangeKind::Unchanged {
                change = ChangeKind::ProviderChanged;
            }

            if change.is_changed() {
                let reason = match change {
                    ChangeKind::Upgraded => {
                        format!("Provider upgraded from {:?} to {:?}", old_ver, new_ver)
                    }
                    ChangeKind::Downgraded => {
                        format!("Provider downgraded from {:?} to {:?}", old_ver, new_ver)
                    }
                    ChangeKind::VersionChanged => format!(
                        "Provider version changed from {:?} to {:?}",
                        old_ver, new_ver
                    ),
                    ChangeKind::DefinitionChanged => {
                        "Alias/function definition modified".to_string()
                    }
                    ChangeKind::ProviderChanged => {
                        "Command provider attribution changed".to_string()
                    }
                    _ => "Command modified".to_string(),
                };

                command_diffs.push(CommandDiff {
                    command_id: id.to_string(),
                    name: new_cmd.name.clone(),
                    change,
                    reason,
                    old_candidate: Some((*old_cmd).clone()),
                    new_candidate: Some((*new_cmd).clone()),
                });
            }
        } else {
            // Added command
            command_diffs.push(CommandDiff {
                command_id: id.to_string(),
                name: new_cmd.name.clone(),
                change: ChangeKind::Added,
                reason: "Newly discovered command".to_string(),
                old_candidate: None,
                new_candidate: Some((*new_cmd).clone()),
            });
        }
    }

    for (&id, &old_cmd) in &from_cmds {
        if !to_cmds.contains_key(id) {
            let eco = old_cmd
                .provider
                .as_ref()
                .map(|p| p.ecosystem.as_str())
                .unwrap_or("path");
            if failed_ecosystems.contains(&eco.to_lowercase()) {
                continue;
            }

            command_diffs.push(CommandDiff {
                command_id: id.to_string(),
                name: old_cmd.name.clone(),
                change: ChangeKind::Removed,
                reason: "Command no longer detected".to_string(),
                old_candidate: Some((*old_cmd).clone()),
                new_candidate: None,
            });
        }
    }

    let is_empty = package_diffs.is_empty() && command_diffs.is_empty();

    SnapshotDiff {
        from_snapshot_id: from.snapshot_id.clone(),
        to_snapshot_id: to.snapshot_id.clone(),
        is_empty,
        packages: package_diffs,
        commands: command_diffs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::AdapterReport;

    #[test]
    fn test_identical_inventories_produce_empty_diff() {
        let inv = Inventory::new("snap-1".into(), true);
        let diff = diff_inventories(&inv, &inv);
        assert!(diff.is_empty);
        assert_eq!(diff.packages.len(), 0);
        assert_eq!(diff.commands.len(), 0);
    }

    #[test]
    fn test_package_upgrade_diff() {
        let mut inv1 = Inventory::new("snap-1".into(), true);
        inv1.packages.push(PackageFact {
            id: "cargo:rg".into(),
            ecosystem: "cargo".into(),
            name: "ripgrep".into(),
            version: Some("14.0.0".into()),
            scope: "global".into(),
            commands: vec!["cargo:rg:rg".into()],
            discovery_method: "cargo".into(),
            confidence: "high".into(),
        });

        let mut inv2 = Inventory::new("snap-2".into(), true);
        inv2.packages.push(PackageFact {
            id: "cargo:rg".into(),
            ecosystem: "cargo".into(),
            name: "ripgrep".into(),
            version: Some("14.1.0".into()),
            scope: "global".into(),
            commands: vec!["cargo:rg:rg".into()],
            discovery_method: "cargo".into(),
            confidence: "high".into(),
        });

        let diff = diff_inventories(&inv1, &inv2);
        assert!(!diff.is_empty);
        assert_eq!(diff.packages.len(), 1);
        assert_eq!(diff.packages[0].change, ChangeKind::Upgraded);
    }

    #[test]
    fn test_failed_adapter_does_not_cause_false_removed() {
        let mut inv1 = Inventory::new("snap-1".into(), true);
        inv1.packages.push(PackageFact {
            id: "npm:pnpm".into(),
            ecosystem: "npm".into(),
            name: "pnpm".into(),
            version: Some("9.0.0".into()),
            scope: "global".into(),
            commands: vec![],
            discovery_method: "npm".into(),
            confidence: "high".into(),
        });

        let mut inv2 = Inventory::new("snap-2".into(), false);
        // npm adapter failed in inv2!
        inv2.adapters.push(AdapterReport {
            name: "npm".into(),
            status: "failed".into(),
            packages_found: 0,
            commands_found: 0,
            error: Some("network error".into()),
        });

        let diff = diff_inventories(&inv1, &inv2);
        // Should NOT consider pnpm as removed because npm adapter failed
        assert!(diff.packages.is_empty());
    }
}
