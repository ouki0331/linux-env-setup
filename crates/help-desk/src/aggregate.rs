//! Inventory aggregation, deduplication, evidence merging, and effective command resolution.

use crate::inventory::{CommandCandidate, Inventory, PackageFact};
use std::collections::{BTreeMap, HashMap};

/// Aggregate discovered packages and commands from multiple adapters into a single canonical Inventory.
pub fn aggregate_inventory(
    mut raw_packages: Vec<PackageFact>,
    mut raw_commands: Vec<CommandCandidate>,
    snapshot_id: String,
    complete: bool,
) -> Inventory {
    // 1. Deduplicate/merge command candidates pointing to the exact same executable path
    let mut path_to_candidate_idx: HashMap<String, usize> = HashMap::new();
    let mut deduped_commands: Vec<CommandCandidate> = Vec::new();

    for cmd in raw_commands.drain(..) {
        if let Some(exe) = &cmd.executable_path {
            if let Some(&existing_idx) = path_to_candidate_idx.get(exe) {
                let existing = &mut deduped_commands[existing_idx];
                // Prefer ecosystem-attributed candidate over generic 'path'
                if existing.kind == "binary" && cmd.kind != "binary" {
                    // Replace existing generic binary with ecosystem candidate
                    *existing = cmd;
                } else {
                    // Merge source files and warnings
                    for sf in cmd.source_files {
                        if !existing.source_files.contains(&sf) {
                            existing.source_files.push(sf);
                        }
                    }
                    for w in cmd.warnings {
                        if !existing.warnings.contains(&w) {
                            existing.warnings.push(w);
                        }
                    }
                }
                continue;
            } else {
                path_to_candidate_idx.insert(exe.clone(), deduped_commands.len());
            }
        }
        deduped_commands.push(cmd);
    }

    // 2. Resolve 'effective' command per name
    // Precedence: alias > function > ecosystem binary (cargo/npm/python/composer) > generic binary
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, cmd) in deduped_commands.iter().enumerate() {
        by_name.entry(cmd.name.clone()).or_default().push(idx);
    }

    for indices in by_name.values() {
        let mut best_idx = None;
        let mut best_score = -1;

        for &idx in indices {
            let cmd = &deduped_commands[idx];
            let score = match cmd.kind.as_str() {
                "alias" => 100,
                "function" => 90,
                "rust_binary" | "npm_cli" | "python_cli" | "composer_cli" => 80,
                "binary" => 50,
                _ => 10,
            };
            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(eff_idx) = best_idx {
            deduped_commands[eff_idx].effective = true;
        }
    }

    // 3. Deduplicate packages by ID
    let mut pkg_map: BTreeMap<String, PackageFact> = BTreeMap::new();
    for pkg in raw_packages.drain(..) {
        pkg_map.entry(pkg.id.clone()).or_insert(pkg);
    }

    // 4. Sort deterministically for reproducible, idempotent snapshots
    let mut packages: Vec<PackageFact> = pkg_map.into_values().collect();
    packages.sort_by(|a, b| a.id.cmp(&b.id));

    deduped_commands.sort_by(|a, b| a.id.cmp(&b.id));

    let mut inv = Inventory::new(snapshot_id, complete);
    inv.packages = packages;
    inv.commands = deduped_commands;
    inv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_command_precedence() {
        let cmds = vec![
            CommandCandidate {
                id: "path:ls".into(),
                name: "ls".into(),
                kind: "binary".into(),
                provider: None,
                executable_path: Some("/bin/ls".into()),
                definition: None,
                source_files: vec![],
                effective: false,
                confidence: "medium".into(),
                discovery_method: "path".into(),
                warnings: vec![],
            },
            CommandCandidate {
                id: "zsh:alias:ls".into(),
                name: "ls".into(),
                kind: "alias".into(),
                provider: None,
                executable_path: None,
                definition: Some("ls -G".into()),
                source_files: vec![],
                effective: false,
                confidence: "high".into(),
                discovery_method: "zsh".into(),
                warnings: vec![],
            },
        ];

        let inv = aggregate_inventory(vec![], cmds, "snap-test".into(), true);
        assert_eq!(inv.commands.len(), 2);

        let alias = inv.commands.iter().find(|c| c.kind == "alias").unwrap();
        let bin = inv.commands.iter().find(|c| c.kind == "binary").unwrap();

        assert!(alias.effective);
        assert!(!bin.effective);
    }

    #[test]
    fn test_merge_same_executable_path() {
        let cmds = vec![
            CommandCandidate {
                id: "path:rg".into(),
                name: "rg".into(),
                kind: "binary".into(),
                provider: None,
                executable_path: Some("/home/u/.cargo/bin/rg".into()),
                definition: None,
                source_files: vec!["/home/u/.cargo/bin/rg".into()],
                effective: false,
                confidence: "medium".into(),
                discovery_method: "path".into(),
                warnings: vec![],
            },
            CommandCandidate {
                id: "cargo:ripgrep:rg".into(),
                name: "rg".into(),
                kind: "rust_binary".into(),
                provider: None,
                executable_path: Some("/home/u/.cargo/bin/rg".into()),
                definition: None,
                source_files: vec![],
                effective: false,
                confidence: "high".into(),
                discovery_method: "cargo".into(),
                warnings: vec![],
            },
        ];

        let inv = aggregate_inventory(vec![], cmds, "snap-test".into(), true);
        assert_eq!(inv.commands.len(), 1);
        assert_eq!(inv.commands[0].id, "cargo:ripgrep:rg");
        assert_eq!(inv.commands[0].kind, "rust_binary");
    }
}
