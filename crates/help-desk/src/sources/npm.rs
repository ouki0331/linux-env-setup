//! npm global packages discovery adapter.

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::exec::run_command;
use crate::inventory::{CommandCandidate, CommandProvider, PackageFact};
use crate::util::normalize_command_name;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct NpmAdapter;

impl SourceAdapter for NpmAdapter {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        // Query global prefix
        let prefix_out = match run_command("npm", &["prefix", "-g"], ctx.timeout, None, &[]) {
            Ok(o) if o.success => o,
            _ => {
                return Ok(AdapterResult::skipped(
                    "npm",
                    "npm command not available in PATH",
                ));
            }
        };

        let prefix_str = prefix_out.stdout.trim();
        if prefix_str.is_empty() {
            return Ok(AdapterResult::skipped(
                "npm",
                "npm prefix -g returned empty output",
            ));
        }

        let prefix_path = PathBuf::from(prefix_str);
        let global_bin_dir = if cfg!(target_os = "windows") {
            prefix_path.clone()
        } else {
            prefix_path.join("bin")
        };

        // Query global packages via npm list -g --depth=0 --json
        let list_out = match run_command(
            "npm",
            &["list", "-g", "--depth=0", "--json"],
            ctx.timeout,
            None,
            &[],
        ) {
            Ok(o) if o.success => o,
            _ => {
                return Ok(AdapterResult::skipped(
                    "npm",
                    "npm list -g --depth=0 --json failed or timed out",
                ));
            }
        };

        let mut packages = Vec::new();
        let mut commands = Vec::new();

        if let Ok(json) = serde_json::from_str::<Value>(&list_out.stdout) {
            parse_npm_global_list(&json, &global_bin_dir, &mut packages, &mut commands);
        }

        Ok(AdapterResult::ok("npm", packages, commands))
    }
}

pub fn parse_npm_global_list(
    json: &Value,
    global_bin_dir: &Path,
    packages: &mut Vec<PackageFact>,
    commands: &mut Vec<CommandCandidate>,
) {
    if let Some(deps) = json.get("dependencies").and_then(|d| d.as_object()) {
        for (pkg_name, info) in deps {
            let ver = info
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let pkg_dir = info
                .get("resolved")
                .and_then(|r| r.as_str())
                .or_else(|| info.get("from").and_then(|f| f.as_str()));

            let mut discovered_bins = Vec::new();

            // Try reading package.json if available
            if let Some(dir_str) = pkg_dir {
                let pkg_json_path = Path::new(dir_str).join("package.json");
                if let Ok(content) = fs::read_to_string(&pkg_json_path) {
                    if let Ok(pj) = serde_json::from_str::<Value>(&content) {
                        extract_bins_from_package_json(&pj, pkg_name, &mut discovered_bins);
                    }
                }
            }

            // Fallback: if package name itself exists as an executable in global bin dir
            if discovered_bins.is_empty() {
                let candidate_bin = global_bin_dir.join(pkg_name);
                if candidate_bin.exists() {
                    discovered_bins.push(pkg_name.clone());
                }
            }

            let mut cmd_ids = Vec::new();
            for bin_name in discovered_bins {
                let norm_cmd = normalize_command_name(&bin_name);
                let cmd_id = format!("npm:{pkg_name}:{norm_cmd}");
                cmd_ids.push(cmd_id.clone());

                let exe_path = global_bin_dir.join(&norm_cmd);
                let exe_str = if exe_path.exists() {
                    Some(exe_path.to_string_lossy().to_string())
                } else {
                    None
                };

                commands.push(CommandCandidate {
                    id: cmd_id,
                    name: norm_cmd,
                    kind: "npm_cli".to_string(),
                    provider: Some(CommandProvider {
                        ecosystem: "npm".to_string(),
                        package: pkg_name.clone(),
                        version: ver.clone(),
                        scope: "global".to_string(),
                    }),
                    executable_path: exe_str,
                    definition: None,
                    source_files: Vec::new(),
                    effective: false,
                    confidence: "high".to_string(),
                    discovery_method: "npm list -g".to_string(),
                    warnings: Vec::new(),
                });
            }

            if !cmd_ids.is_empty() {
                packages.push(PackageFact {
                    id: format!("npm:{pkg_name}"),
                    ecosystem: "npm".to_string(),
                    name: pkg_name.clone(),
                    version: ver,
                    scope: "global".to_string(),
                    commands: cmd_ids,
                    discovery_method: "npm list -g".to_string(),
                    confidence: "high".to_string(),
                });
            }
        }
    }
}

pub fn extract_bins_from_package_json(pj: &Value, pkg_name: &str, out_bins: &mut Vec<String>) {
    if let Some(bin) = pj.get("bin") {
        match bin {
            Value::String(_) => {
                out_bins.push(pkg_name.to_string());
            }
            Value::Object(map) => {
                for k in map.keys() {
                    out_bins.push(k.clone());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_extract_bins_string_and_object() {
        let mut bins1 = Vec::new();
        let pj1 = json!({ "bin": "./bin/run.js" });
        extract_bins_from_package_json(&pj1, "my-tool", &mut bins1);
        assert_eq!(bins1, vec!["my-tool"]);

        let mut bins2 = Vec::new();
        let pj2 = json!({ "bin": { "app1": "./app1.js", "app2": "./app2.js" } });
        extract_bins_from_package_json(&pj2, "multi-tool", &mut bins2);
        assert_eq!(bins2.len(), 2);
        assert!(bins2.contains(&"app1".to_string()));
        assert!(bins2.contains(&"app2".to_string()));
    }

    #[test]
    fn test_parse_npm_global_list() {
        let dir = tempdir().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("pnpm"), b"#!/bin/sh\n").unwrap();

        let sample = json!({
            "dependencies": {
                "pnpm": {
                    "version": "9.1.0"
                }
            }
        });

        let mut pkgs = Vec::new();
        let mut cmds = Vec::new();
        parse_npm_global_list(&sample, &bin_dir, &mut pkgs, &mut cmds);

        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "pnpm");
        assert_eq!(pkgs[0].version.as_deref(), Some("9.1.0"));
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "pnpm");
    }
}
