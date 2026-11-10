//! PATH and user binary directory scanner.

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::inventory::{CommandCandidate, PackageFact};
use crate::util::normalize_command_name;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub struct PathAdapter;

impl SourceAdapter for PathAdapter {
    fn name(&self) -> &'static str {
        "path"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        let scan_dirs = ctx.config.effective_scan_dirs(&ctx.paths.home);
        let mut commands = Vec::new();
        let mut packages = Vec::new();
        let mut seen_paths = HashSet::new();

        for dir in scan_dirs {
            if !dir.is_dir() {
                continue;
            }

            // Exclude project-level dirs
            let dir_str = dir.to_string_lossy();
            if dir_str.contains("/.venv/")
                || dir_str.contains("/node_modules/")
                || dir_str.contains("/vendor/")
                || dir_str.contains("/target/")
            {
                continue;
            }

            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();

                // Skip non-files/non-symlinks or hidden files
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with('.') {
                    continue;
                }

                // Check executable bit
                if !is_executable_file(&path) {
                    continue;
                }

                // Canonicalize or resolve symlink
                let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                let canonical_str = canonical.to_string_lossy().to_string();

                if !seen_paths.insert(canonical_str.clone()) {
                    continue; // Already processed this physical file
                }

                let cmd_name = normalize_command_name(&file_name_str);
                let id = format!("path:{cmd_name}");

                // Register candidate
                commands.push(CommandCandidate {
                    id: id.clone(),
                    name: cmd_name.clone(),
                    kind: "binary".to_string(),
                    provider: None,
                    executable_path: Some(canonical_str),
                    definition: None,
                    source_files: vec![path.to_string_lossy().to_string()],
                    effective: false, // will be resolved in aggregation
                    confidence: "medium".to_string(),
                    discovery_method: "path scan".to_string(),
                    warnings: Vec::new(),
                });

                // Record package fact for un-attributed binaries (version: null)
                packages.push(PackageFact {
                    id: format!("path:{cmd_name}"),
                    ecosystem: "path".to_string(),
                    name: cmd_name.clone(),
                    version: None,
                    scope: "user".to_string(),
                    commands: vec![id],
                    discovery_method: "path scan".to_string(),
                    confidence: "medium".to_string(),
                });
            }
        }

        Ok(AdapterResult::ok("path", packages, commands))
    }
}

fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        if let Ok(meta) = fs::metadata(path) {
            if meta.is_file() {
                return (meta.permissions().mode() & 0o111) != 0;
            }
        }
        false
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::paths::Paths;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn test_path_scanner_discovers_executable() {
        let dir = tempdir().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        let script_path = bin_dir.join("my-tool");
        fs::write(&script_path, b"#!/bin/sh\necho hi\n").unwrap();

        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let mut config = Config::default();
        config.scan_dirs.push(bin_dir);

        let paths = Paths {
            home: dir.path().to_path_buf(),
            data_dir: dir.path().join("data"),
            config_dir: dir.path().join("config"),
        };

        let ctx = ScanContext {
            paths: &paths,
            config: &config,
            allow_source: None,
            zsh_stdin: None,
            timeout: Duration::from_secs(5),
        };

        let adapter = PathAdapter;
        let res = adapter.scan(&ctx).unwrap();
        assert!(res
            .commands
            .iter()
            .any(|c| c.name == "my-tool" && c.kind == "binary"));
    }
}
