//! Cargo global install discovery adapter (`cargo install --list`).

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::exec::run_command;
use crate::inventory::{CommandCandidate, CommandProvider, PackageFact};
use crate::util::normalize_command_name;
use std::path::Path;

pub struct CargoAdapter;

impl SourceAdapter for CargoAdapter {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        let out = match run_command("cargo", &["install", "--list"], ctx.timeout, None, &[]) {
            Ok(o) => o,
            Err(_) => {
                return Ok(AdapterResult::skipped(
                    "cargo",
                    "cargo command not available in PATH",
                ));
            }
        };

        if !out.success {
            return Ok(AdapterResult::failed(
                "cargo",
                format!(
                    "cargo install --list exited with status {:?}",
                    out.exit_code
                ),
            ));
        }

        let cargo_bin_dir = ctx.paths.home.join(".cargo/bin");
        let (packages, commands) = parse_cargo_install_list(&out.stdout, &cargo_bin_dir);

        Ok(AdapterResult::ok("cargo", packages, commands))
    }
}

pub fn parse_cargo_install_list(
    output: &str,
    cargo_bin_dir: &Path,
) -> (Vec<PackageFact>, Vec<CommandCandidate>) {
    let mut packages = Vec::new();
    let mut commands = Vec::new();

    let mut current_crate: Option<(String, Option<String>)> = None;
    let mut current_cmds = Vec::new();

    for line in output.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Indented binary name, e.g. `    rg:` or `    cargo-watch:`
            let bin_raw = line.trim().trim_end_matches(':');
            if !bin_raw.is_empty() {
                let bin_name = normalize_command_name(bin_raw);
                let (crate_name, crate_ver) = match &current_crate {
                    Some(c) => c.clone(),
                    None => (bin_name.clone(), None),
                };

                let cmd_id = format!("cargo:{crate_name}:{bin_name}");
                current_cmds.push(cmd_id.clone());

                let exe_path = cargo_bin_dir.join(&bin_name);
                let exe_str = if exe_path.exists() {
                    Some(exe_path.to_string_lossy().to_string())
                } else {
                    None
                };

                commands.push(CommandCandidate {
                    id: cmd_id,
                    name: bin_name,
                    kind: "rust_binary".to_string(),
                    provider: Some(CommandProvider {
                        ecosystem: "cargo".to_string(),
                        package: crate_name,
                        version: crate_ver,
                        scope: "global".to_string(),
                    }),
                    executable_path: exe_str,
                    definition: None,
                    source_files: Vec::new(),
                    effective: false,
                    confidence: "high".to_string(),
                    discovery_method: "cargo install --list".to_string(),
                    warnings: Vec::new(),
                });
            }
        } else if !line.trim().is_empty() {
            // New crate header, e.g. `ripgrep v14.1.1:` or `tokio-console v0.1.12 (git+https://...):`
            if let Some((crate_name, crate_ver)) = current_crate.take() {
                packages.push(PackageFact {
                    id: format!("cargo:{crate_name}"),
                    ecosystem: "cargo".to_string(),
                    name: crate_name,
                    version: crate_ver,
                    scope: "global".to_string(),
                    commands: current_cmds.clone(),
                    discovery_method: "cargo install --list".to_string(),
                    confidence: "high".to_string(),
                });
                current_cmds.clear();
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(&name) = parts.first() {
                let ver = parts
                    .get(1)
                    .map(|v| v.trim_start_matches('v').trim_end_matches(':').to_string());
                current_crate = Some((name.to_string(), ver));
            }
        }
    }

    if let Some((crate_name, crate_ver)) = current_crate {
        packages.push(PackageFact {
            id: format!("cargo:{crate_name}"),
            ecosystem: "cargo".to_string(),
            name: crate_name,
            version: crate_ver,
            scope: "global".to_string(),
            commands: current_cmds,
            discovery_method: "cargo install --list".to_string(),
            confidence: "high".to_string(),
        });
    }

    (packages, commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_cargo_install_list() {
        let sample = r#"
ripgrep v14.1.1:
    rg
cargo-watch v8.5.2 (https://github.com/watchexec/cargo-watch#...):
    cargo-watch
"#;
        let dir = tempdir().unwrap();
        let (pkgs, cmds) = parse_cargo_install_list(sample, dir.path());

        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "ripgrep");
        assert_eq!(pkgs[0].version.as_deref(), Some("14.1.1"));
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "rg");
        assert_eq!(cmds[1].name, "cargo-watch");
    }
}
