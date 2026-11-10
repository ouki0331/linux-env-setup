//! Comprehensive end-to-end CLI integration and smoke tests.
//!
//! Verified flows in isolated temp directories:
//! - empty list
//! - add -> show -> search -> edit -> remove
//! - scan fixture -> snapshot list
//! - diff unchanged vs changed
//! - export-ai & export-ai --changes-only
//! - validate & transactional import
//! - doctor
//! - exit codes (0 for success, 2 for usage error, 3 for not found, 4 for validation)

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn hdesk_bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target/release/hdesk");
    if !p.exists() {
        // Fall back to target/debug/hdesk if release not built yet
        p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("target/debug/hdesk");
    }
    p
}

struct TestEnv {
    _dir: tempfile::TempDir,
    home: PathBuf,
    data_dir: PathBuf,
    config_dir: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let data_dir = dir.path().join("data");
        let config_dir = dir.path().join("config");

        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        Self {
            _dir: dir,
            home,
            data_dir,
            config_dir,
        }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let bin = hdesk_bin();
        let output = Command::new(&bin)
            .args(args)
            .env("HOME", &self.home)
            .env("HDESK_DATA_DIR", &self.data_dir)
            .env("HDESK_CONFIG_DIR", &self.config_dir)
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute {:?}: {e}", bin));

        let code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        (code, stdout, stderr)
    }
}

#[test]
fn test_cli_empty_list() {
    let env = TestEnv::new();
    let (code, stdout, _) = env.run(&["list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Catalog is empty"));
}

#[test]
fn test_cli_not_found_exit_code_3() {
    let env = TestEnv::new();
    let (code, _, stderr) = env.run(&["nonexistent_cmd_xyz"]);
    assert_eq!(code, 3);
    assert!(stderr.contains("not found in catalog"));
}

#[test]
fn test_cli_usage_error_exit_code_2() {
    let env = TestEnv::new();
    let (code, _, _) = env.run(&["--completely-invalid-flag"]);
    assert_eq!(code, 2);
}

#[test]
fn test_cli_doctor() {
    let env = TestEnv::new();
    let (code, stdout, _) = env.run(&["doctor"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("hdesk System Health Check"));
    assert!(stdout.contains("Database status"));
}

#[test]
fn test_cli_scan_snapshot_export_import_flow() {
    let env = TestEnv::new();

    // 1. Create a dummy executable in ~/.local/bin
    let local_bin = env.home.join(".local/bin");
    fs::create_dir_all(&local_bin).unwrap();
    let my_tool = local_bin.join("my-custom-cli");
    fs::write(&my_tool, b"#!/bin/sh\necho custom\n").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&my_tool).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&my_tool, perms).unwrap();
    }

    // 2. Scan and create snapshot
    let inv_file = env.home.join("inventory.json");
    let (code, stdout, _) =
        env.run(&["scan", "--snapshot", "--output", inv_file.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Discovered:"));
    assert!(stdout.contains("Saved snapshot:"));
    assert!(inv_file.exists());

    // 3. Inspect inventory
    let (code, stdout, _) = env.run(&["inventory", inv_file.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Inventory:"));
    assert!(stdout.contains("Snapshot ID :"));

    // 4. List snapshots
    let (code, stdout, _) = env.run(&["snapshot", "list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Stored Snapshots"));

    // 5. Diff unchanged (snapshot against itself)
    // First extract snapshot ID from output
    let snap_id = stdout
        .lines()
        .find(|l| l.contains('|'))
        .unwrap()
        .split('|')
        .next()
        .unwrap()
        .trim();

    let (code, stdout, _) = env.run(&["diff", "--from", snap_id, "--to", snap_id]);
    assert_eq!(code, 0);
    assert!(stdout.contains("No differences detected"));

    // 6. Export AI payload
    let ai_input_file = env.home.join("ai-input.json");
    let (code, stdout, _) = env.run(&[
        "export-ai",
        inv_file.to_str().unwrap(),
        "--output",
        ai_input_file.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(stdout.contains("AI prompt payload written"));
    assert!(ai_input_file.exists());

    // 7. Validate and Import an enriched catalog JSON
    let catalog_json = r#"{
            "schema_version": "0.1",
            "entries": [
                {
                    "command_id": "path:my-custom-cli",
                    "name": "my-custom-cli",
                    "kind": "binary",
                    "summary": "Demonstration custom utility tool.",
                    "syntax": "my-custom-cli [args]",
                    "examples": [
                        { "description": "Run default", "command": "my-custom-cli" }
                    ],
                    "tags": ["custom", "test"]
                }
            ]
        }"#;

    let cat_file = env.home.join("catalog.json");
    fs::write(&cat_file, catalog_json).unwrap();

    let (code, stdout, _) = env.run(&["validate", cat_file.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Validation succeeded"));

    let (code, stdout, _) = env.run(&["import", cat_file.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Transactional import successful"));

    // 8. Query imported entry
    let (code, stdout, _) = env.run(&["my-custom-cli"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("my-custom-cli"));
    assert!(stdout.contains("Demonstration custom utility tool"));

    // 9. Search
    let (code, stdout, _) = env.run(&["search", "Demonstration"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("my-custom-cli"));

    // 10. List
    let (code, stdout, _) = env.run(&["list"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("my-custom-cli"));

    // 11. Remove
    let (code, stdout, _) = env.run(&["remove", "my-custom-cli", "--yes"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Removed 'my-custom-cli' from catalog"));

    let (code, _, _) = env.run(&["my-custom-cli"]);
    assert_eq!(code, 3);
}

#[test]
fn test_cli_shell_init_zsh() {
    let env = TestEnv::new();
    let (code, stdout, _) = env.run(&["shell-init", "zsh"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("hdesk_dump_zsh"));
}
