//! PHP Composer global CLI discovery adapter.

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::exec::run_command;
use crate::inventory::{CommandCandidate, CommandProvider, PackageFact};
use crate::util::normalize_command_name;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ComposerAdapter;

impl SourceAdapter for ComposerAdapter {
    fn name(&self) -> &'static str {
        "composer"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        // Query global bin-dir
        let bin_out = match run_command(
            "composer",
            &["global", "config", "bin-dir", "--absolute"],
            ctx.timeout,
            None,
            &[],
        ) {
            Ok(o) if o.success => o,
            _ => {
                return Ok(AdapterResult::skipped(
                    "composer",
                    "composer command not available in PATH",
                ));
            }
        };

        let bin_dir_str = bin_out.stdout.trim();
        if bin_dir_str.is_empty() {
            return Ok(AdapterResult::skipped(
                "composer",
                "composer global bin-dir returned empty output",
            ));
        }

        let global_bin_dir = PathBuf::from(bin_dir_str);
        if !global_bin_dir.is_dir() {
            return Ok(AdapterResult::ok("composer", Vec::new(), Vec::new()));
        }

        // Query installed global packages: composer global show --format=json
        let show_out = run_command(
            "composer",
            &["global", "show", "--format=json"],
            ctx.timeout,
            None,
            &[],
        );

        let mut packages = Vec::new();
        let mut commands = Vec::new();

        if let Ok(out) = show_out {
            if out.success {
                if let Ok(json) = serde_json::from_str::<Value>(&out.stdout) {
                    parse_composer_global_json(
                        &json,
                        &global_bin_dir,
                        &mut packages,
                        &mut commands,
                    );
                }
            }
        }

        // If package json parsing found nothing, inspect the binaries directly in bin_dir
        if commands.is_empty() {
            if let Ok(entries) = fs::read_dir(&global_bin_dir) {
                for e in entries.flatten() {
                    let path = e.path();
                    if path.is_file() {
                        let name = normalize_command_name(&e.file_name().to_string_lossy());
                        let cmd_id = format!("composer:global:{name}");

                        commands.push(CommandCandidate {
                            id: cmd_id.clone(),
                            name: name.clone(),
                            kind: "composer_cli".to_string(),
                            provider: Some(CommandProvider {
                                ecosystem: "composer".to_string(),
                                package: "global".to_string(),
                                version: None,
                                scope: "global".to_string(),
                            }),
                            executable_path: Some(path.to_string_lossy().to_string()),
                            definition: None,
                            source_files: Vec::new(),
                            effective: false,
                            confidence: "medium".to_string(),
                            discovery_method: "composer global bin-dir".to_string(),
                            warnings: Vec::new(),
                        });

                        packages.push(PackageFact {
                            id: format!("composer:{name}"),
                            ecosystem: "composer".to_string(),
                            name,
                            version: None,
                            scope: "global".to_string(),
                            commands: vec![cmd_id],
                            discovery_method: "composer global bin-dir".to_string(),
                            confidence: "medium".to_string(),
                        });
                    }
                }
            }
        }

        Ok(AdapterResult::ok("composer", packages, commands))
    }
}

pub fn parse_composer_global_json(
    json: &Value,
    global_bin_dir: &Path,
    packages: &mut Vec<PackageFact>,
    commands: &mut Vec<CommandCandidate>,
) {
    if let Some(installed) = json.get("installed").and_then(|i| i.as_array()) {
        for pkg in installed {
            let name_raw = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let ver = pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if name_raw.is_empty() {
                continue;
            }

            // Vendor/package convention in composer
            let short_name = name_raw.split('/').next_back().unwrap_or(name_raw);
            let bin_candidate = global_bin_dir.join(short_name);

            let mut cmd_ids = Vec::new();
            if bin_candidate.exists() {
                let cmd_name = normalize_command_name(short_name);
                let cmd_id = format!("composer:{name_raw}:{cmd_name}");
                cmd_ids.push(cmd_id.clone());

                commands.push(CommandCandidate {
                    id: cmd_id,
                    name: cmd_name,
                    kind: "composer_cli".to_string(),
                    provider: Some(CommandProvider {
                        ecosystem: "composer".to_string(),
                        package: name_raw.to_string(),
                        version: ver.clone(),
                        scope: "global".to_string(),
                    }),
                    executable_path: Some(bin_candidate.to_string_lossy().to_string()),
                    definition: None,
                    source_files: Vec::new(),
                    effective: false,
                    confidence: "high".to_string(),
                    discovery_method: "composer global show".to_string(),
                    warnings: Vec::new(),
                });
            }

            if !cmd_ids.is_empty() {
                packages.push(PackageFact {
                    id: format!("composer:{name_raw}"),
                    ecosystem: "composer".to_string(),
                    name: name_raw.to_string(),
                    version: ver,
                    scope: "global".to_string(),
                    commands: cmd_ids,
                    discovery_method: "composer global show".to_string(),
                    confidence: "high".to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_parse_composer_global_json() {
        let dir = tempdir().unwrap();
        let bin_path = dir.path().join("psalm");
        fs::write(&bin_path, b"#!/usr/bin/env php\n").unwrap();

        let sample = json!({
            "installed": [
                {
                    "name": "vimeo/psalm",
                    "version": "5.24.0"
                }
            ]
        });

        let mut pkgs = Vec::new();
        let mut cmds = Vec::new();
        parse_composer_global_json(&sample, dir.path(), &mut pkgs, &mut cmds);

        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "vimeo/psalm");
        assert_eq!(pkgs[0].version.as_deref(), Some("5.24.0"));
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "psalm");
    }
}
