//! Zsh alias and user function discovery adapter.

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::exec::run_command;
use crate::inventory::{CommandCandidate, PackageFact};
use crate::util::{normalize_command_name, sha256_str};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct ZshAdapter;

impl SourceAdapter for ZshAdapter {
    fn name(&self) -> &'static str {
        "zsh"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        let mut commands = Vec::new();
        let mut packages = Vec::new();

        // 1. If explicit stdin was piped from `hdesk shell-init zsh` runtime, parse it
        if let Some(stdin_content) = &ctx.zsh_stdin {
            if let Ok(mut parsed) = parse_zsh_runtime_dump(stdin_content, ctx) {
                commands.append(&mut parsed.0);
                packages.append(&mut parsed.1);
                return Ok(AdapterResult::ok("zsh", packages, commands));
            }
        }

        // 2. If --allow-source <file> is provided, run isolated subshell
        if let Some(source_file) = &ctx.allow_source {
            eprintln!(
                "⚠️ Warning: Running isolated Zsh subshell to source '{}'.",
                source_file.display()
            );
            match run_isolated_zsh_source(source_file, ctx.timeout) {
                Ok(output) => {
                    if let Ok(mut parsed) = parse_zsh_runtime_dump(&output, ctx) {
                        commands.append(&mut parsed.0);
                        packages.append(&mut parsed.1);
                        return Ok(AdapterResult::ok("zsh", packages, commands));
                    }
                }
                Err(e) => {
                    return Ok(AdapterResult::failed(
                        "zsh",
                        format!("Failed to source file '{}': {e}", source_file.display()),
                    ));
                }
            }
        }

        // 3. Static safe scan of user Zsh files (default mode, no execution!)
        let user_zsh_files =
            discover_user_zsh_files(&ctx.paths.home, &ctx.config.allowed_function_dirs);
        for file in user_zsh_files {
            if let Ok(content) = fs::read_to_string(&file) {
                let file_str = file.to_string_lossy().to_string();
                let (mut file_cmds, mut file_pkgs) =
                    parse_zsh_script_statically(&content, &file_str);
                commands.append(&mut file_cmds);
                packages.append(&mut file_pkgs);
            }
        }

        Ok(AdapterResult::ok("zsh", packages, commands))
    }
}

/// Shell-init script for user to evaluate: `eval "$(hdesk shell-init zsh)"`
pub fn generate_shell_init_script() -> &'static str {
    r#"# hdesk Zsh integration
# This script dumps current user-defined aliases and functions to stdout without executing them.
hdesk_dump_zsh() {
    echo "=== ALIASES ==="
    alias -L
    echo "=== FUNCTIONS ==="
    typeset +f -m '*'
}
"#
}

/// Discover user configuration files without touching system or framework scripts.
fn discover_user_zsh_files(home: &Path, extra_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let candidates = [
        home.join(".zshrc"),
        home.join(".zshenv"),
        home.join(".zprofile"),
        home.join(".config/zsh/.zshrc"),
    ];

    for c in candidates {
        if c.is_file() {
            files.push(c);
        }
    }

    for d in extra_dirs {
        let expanded = crate::paths::expand_tilde(d, home);
        if expanded.is_file() {
            files.push(expanded);
        } else if expanded.is_dir() {
            if let Ok(entries) = fs::read_dir(&expanded) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_file() && p.extension().is_some_and(|ext| ext == "zsh") {
                        files.push(p);
                    }
                }
            }
        }
    }

    files
}

/// Run an isolated zsh subshell to safely source a single file.
fn run_isolated_zsh_source(file: &Path, timeout: Duration) -> Result<String> {
    let script = format!(
        "source {}\nalias -L\n",
        shell_escape::escape(file.to_string_lossy())
    );
    let output = run_command("zsh", &["-f", "-c", &script], timeout, None, &[])?;
    if !output.success {
        return Err(crate::error::Error::Other(format!(
            "Zsh exit code {}: {}",
            output.exit_code.unwrap_or(1),
            output.stderr
        )));
    }
    Ok(output.stdout)
}

/// Parse zsh static content (lines like `alias foo="bar"` or `function myfunc() { ... }`).
pub fn parse_zsh_script_statically(
    content: &str,
    source_file: &str,
) -> (Vec<CommandCandidate>, Vec<PackageFact>) {
    let mut commands = Vec::new();
    let mut packages = Vec::new();
    let mut seen = HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // 1. Alias parsing: `alias name='...'` or `alias name="..."`
        if trimmed.starts_with("alias ") {
            let rest = trimmed.strip_prefix("alias ").unwrap().trim();
            if let Some((name_part, val_part)) = rest.split_once('=') {
                let name = normalize_command_name(name_part.trim_matches(['\'', '"']));
                if !name.is_empty() && is_valid_ident(&name) && seen.insert((name.clone(), "alias"))
                {
                    let expansion = val_part.trim_matches(['\'', '"']).to_string();
                    let def_hash = sha256_str(&expansion);
                    let id = format!("zsh:alias:{name}");

                    commands.push(CommandCandidate {
                        id: id.clone(),
                        name: name.clone(),
                        kind: "alias".to_string(),
                        provider: None,
                        executable_path: None,
                        definition: Some(expansion),
                        source_files: vec![source_file.to_string()],
                        effective: false,
                        confidence: "high".to_string(),
                        discovery_method: "zsh config file".to_string(),
                        warnings: Vec::new(),
                    });

                    packages.push(PackageFact {
                        id: id.clone(),
                        ecosystem: "zsh".to_string(),
                        name: name.clone(),
                        version: Some(format!("hash:{:.8}", def_hash)),
                        scope: "user".to_string(),
                        commands: vec![id],
                        discovery_method: "zsh config file".to_string(),
                        confidence: "high".to_string(),
                    });
                }
            }
        }

        // 2. Function header parsing: `my_func() {` or `function my_func {`
        if trimmed.contains("()") || trimmed.starts_with("function ") {
            if let Some(func_name) = extract_func_name(trimmed) {
                let name = normalize_command_name(&func_name);
                if !is_framework_or_plugin_func(&name) && seen.insert((name.clone(), "function")) {
                    let id = format!("zsh:function:{name}");

                    commands.push(CommandCandidate {
                        id: id.clone(),
                        name: name.clone(),
                        kind: "function".to_string(),
                        provider: None,
                        executable_path: None,
                        definition: Some(format!("User function in {source_file}")),
                        source_files: vec![source_file.to_string()],
                        effective: false,
                        confidence: "high".to_string(),
                        discovery_method: "zsh config file".to_string(),
                        warnings: Vec::new(),
                    });

                    packages.push(PackageFact {
                        id: id.clone(),
                        ecosystem: "zsh".to_string(),
                        name: name.clone(),
                        version: None,
                        scope: "user".to_string(),
                        commands: vec![id],
                        discovery_method: "zsh config file".to_string(),
                        confidence: "high".to_string(),
                    });
                }
            }
        }
    }

    (commands, packages)
}

fn extract_func_name(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("function ") {
        let name = rest.split(['(', '{', ' ', '\t']).next()?.trim();
        if is_valid_ident(name) {
            return Some(name.to_string());
        }
    } else if let Some((before_paren, _)) = line.split_once("()") {
        let name = before_paren.split_whitespace().last()?.trim();
        if is_valid_ident(name) {
            return Some(name.to_string());
        }
    }
    None
}

fn is_valid_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

pub fn is_framework_or_plugin_func(name: &str) -> bool {
    name.starts_with('_')
        || name.starts_with("omz_")
        || name.starts_with("comp")
        || name.starts_with("zsh_")
        || name.starts_with("prompt_")
        || name.starts_with("vcs_info")
        || name.contains("::")
}

fn parse_zsh_runtime_dump(
    dump: &str,
    _ctx: &ScanContext<'_>,
) -> Result<(Vec<CommandCandidate>, Vec<PackageFact>)> {
    Ok(parse_zsh_script_statically(dump, "zsh runtime"))
}

mod shell_escape {
    use std::borrow::Cow;
    pub fn escape(s: Cow<'_, str>) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_zsh_script_statically() {
        let script = r#"
# aliases
alias ll='ls -la'
alias gst="git status"

# function
deploy_prod() {
    echo "deploying"
}

function backup_db {
    echo "backup"
}

_omz_plugin_helper() {
    echo "framework"
}
"#;

        let (cmds, pkgs) = parse_zsh_script_statically(script, "~/.zshrc");
        assert_eq!(cmds.len(), 4); // ll, gst, deploy_prod, backup_db (_omz excluded)
        assert_eq!(pkgs.len(), 4);

        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ll"));
        assert!(names.contains(&"gst"));
        assert!(names.contains(&"deploy_prod"));
        assert!(names.contains(&"backup_db"));
        assert!(!names.contains(&"_omz_plugin_helper"));
    }
}
