//! Python global CLI discovery (pipx, uv tools, global console_scripts).

use super::{AdapterResult, ScanContext, SourceAdapter};
use crate::error::Result;
use crate::exec::run_command;
use crate::inventory::{CommandCandidate, CommandProvider, PackageFact};
use crate::util::normalize_command_name;
use serde_json::Value;

pub struct PythonAdapter;

impl SourceAdapter for PythonAdapter {
    fn name(&self) -> &'static str {
        "python"
    }

    fn scan(&self, ctx: &ScanContext<'_>) -> Result<AdapterResult> {
        let mut packages = Vec::new();
        let mut commands = Vec::new();
        let mut found_any = false;

        // 1. Try pipx list --output json
        if let Ok(out) = run_command(
            "pipx",
            &["list", "--output", "json"],
            ctx.timeout,
            None,
            &[],
        ) {
            if out.success {
                found_any = true;
                if let Ok(json) = serde_json::from_str::<Value>(&out.stdout) {
                    parse_pipx_json(&json, &mut packages, &mut commands);
                }
            }
        }

        // 2. Try uv tool list
        if let Ok(out) = run_command("uv", &["tool", "list"], ctx.timeout, None, &[]) {
            if out.success {
                found_any = true;
                parse_uv_tool_list(&out.stdout, &mut packages, &mut commands);
            }
        }

        // 3. Fallback: query python3 importlib.metadata for user/global console_scripts
        if packages.is_empty() {
            let py_script = r#"
import sys, json
# Skip if inside a virtual environment
if sys.prefix != sys.base_prefix:
    print("{}")
    sys.exit(0)

try:
    from importlib.metadata import distributions, entry_points
    res = {}
    for dist in distributions():
        name = dist.metadata["Name"]
        ver = dist.version
        # Get console_scripts
        eps = [ep.name for ep in dist.entry_points if ep.group == "console_scripts"]
        if eps:
            res[name] = {"version": ver, "commands": eps}
    print(json.dumps(res))
except Exception:
    print("{}")
"#;
            if let Ok(out) = run_command("python3", &["-c", py_script], ctx.timeout, None, &[]) {
                if out.success {
                    found_any = true;
                    if let Ok(map) = serde_json::from_str::<Value>(&out.stdout) {
                        parse_importlib_json(&map, &mut packages, &mut commands);
                    }
                }
            }
        }

        if !found_any && packages.is_empty() {
            return Ok(AdapterResult::skipped(
                "python",
                "No global Python tool runner (pipx, uv, or python3) detected",
            ));
        }

        Ok(AdapterResult::ok("python", packages, commands))
    }
}

pub fn parse_pipx_json(
    json: &Value,
    packages: &mut Vec<PackageFact>,
    commands: &mut Vec<CommandCandidate>,
) {
    if let Some(venvs) = json.get("venvs").and_then(|v| v.as_object()) {
        for (pkg_name, info) in venvs {
            let metadata = info.get("metadata");
            let ver = metadata
                .and_then(|m| m.get("main_package"))
                .and_then(|p| p.get("package_version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut app_names = Vec::new();
            if let Some(apps) = metadata
                .and_then(|m| m.get("main_package"))
                .and_then(|p| p.get("apps"))
                .and_then(|a| a.as_array())
            {
                for app in apps {
                    if let Some(name) = app.as_str() {
                        let cmd_name = normalize_command_name(name);
                        let cmd_id = format!("python:{pkg_name}:{cmd_name}");
                        app_names.push(cmd_id.clone());

                        commands.push(CommandCandidate {
                            id: cmd_id,
                            name: cmd_name,
                            kind: "python_cli".to_string(),
                            provider: Some(CommandProvider {
                                ecosystem: "python".to_string(),
                                package: pkg_name.to_string(),
                                version: ver.clone(),
                                scope: "global".to_string(),
                            }),
                            executable_path: None,
                            definition: None,
                            source_files: Vec::new(),
                            effective: false,
                            confidence: "high".to_string(),
                            discovery_method: "pipx list".to_string(),
                            warnings: Vec::new(),
                        });
                    }
                }
            }

            packages.push(PackageFact {
                id: format!("python:{pkg_name}"),
                ecosystem: "python".to_string(),
                name: pkg_name.to_string(),
                version: ver,
                scope: "global".to_string(),
                commands: app_names,
                discovery_method: "pipx list".to_string(),
                confidence: "high".to_string(),
            });
        }
    }
}

pub fn parse_uv_tool_list(
    output: &str,
    packages: &mut Vec<PackageFact>,
    commands: &mut Vec<CommandCandidate>,
) {
    // Format: `ruff v0.4.1` with lines underneath `- ruff`
    let mut current_pkg: Option<(String, Option<String>)> = None;
    let mut current_cmds = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('-') {
            // Executable: `- ruff`
            if let Some(exe) = trimmed.strip_prefix('-').map(|s| s.trim()) {
                let (pkg_name, ver) = match &current_pkg {
                    Some(p) => p.clone(),
                    None => (exe.to_string(), None),
                };
                let cmd_name = normalize_command_name(exe);
                let cmd_id = format!("python:{pkg_name}:{cmd_name}");
                current_cmds.push(cmd_id.clone());

                commands.push(CommandCandidate {
                    id: cmd_id,
                    name: cmd_name,
                    kind: "python_cli".to_string(),
                    provider: Some(CommandProvider {
                        ecosystem: "python".to_string(),
                        package: pkg_name,
                        version: ver,
                        scope: "global".to_string(),
                    }),
                    executable_path: None,
                    definition: None,
                    source_files: Vec::new(),
                    effective: false,
                    confidence: "high".to_string(),
                    discovery_method: "uv tool list".to_string(),
                    warnings: Vec::new(),
                });
            }
        } else if !trimmed.is_empty() {
            // New package header, e.g. `black v24.4.2`
            if let Some((pkg_name, ver)) = current_pkg.take() {
                packages.push(PackageFact {
                    id: format!("python:{pkg_name}"),
                    ecosystem: "python".to_string(),
                    name: pkg_name,
                    version: ver,
                    scope: "global".to_string(),
                    commands: current_cmds.clone(),
                    discovery_method: "uv tool list".to_string(),
                    confidence: "high".to_string(),
                });
                current_cmds.clear();
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if let Some(&name) = parts.first() {
                let ver = parts.get(1).map(|v| v.trim_start_matches('v').to_string());
                current_pkg = Some((name.to_string(), ver));
            }
        }
    }

    if let Some((pkg_name, ver)) = current_pkg {
        packages.push(PackageFact {
            id: format!("python:{pkg_name}"),
            ecosystem: "python".to_string(),
            name: pkg_name,
            version: ver,
            scope: "global".to_string(),
            commands: current_cmds,
            discovery_method: "uv tool list".to_string(),
            confidence: "high".to_string(),
        });
    }
}

fn parse_importlib_json(
    map: &Value,
    packages: &mut Vec<PackageFact>,
    commands: &mut Vec<CommandCandidate>,
) {
    if let Some(obj) = map.as_object() {
        for (pkg_name, data) in obj {
            let ver = data
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut cmd_ids = Vec::new();

            if let Some(eps) = data.get("commands").and_then(|c| c.as_array()) {
                for ep in eps {
                    if let Some(cmd_str) = ep.as_str() {
                        let cmd_name = normalize_command_name(cmd_str);
                        let cmd_id = format!("python:{pkg_name}:{cmd_name}");
                        cmd_ids.push(cmd_id.clone());

                        commands.push(CommandCandidate {
                            id: cmd_id,
                            name: cmd_name,
                            kind: "python_cli".to_string(),
                            provider: Some(CommandProvider {
                                ecosystem: "python".to_string(),
                                package: pkg_name.clone(),
                                version: ver.clone(),
                                scope: "global".to_string(),
                            }),
                            executable_path: None,
                            definition: None,
                            source_files: Vec::new(),
                            effective: false,
                            confidence: "high".to_string(),
                            discovery_method: "python importlib console_scripts".to_string(),
                            warnings: Vec::new(),
                        });
                    }
                }
            }

            packages.push(PackageFact {
                id: format!("python:{pkg_name}"),
                ecosystem: "python".to_string(),
                name: pkg_name.clone(),
                version: ver,
                scope: "global".to_string(),
                commands: cmd_ids,
                discovery_method: "python importlib console_scripts".to_string(),
                confidence: "high".to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_pipx_json() {
        let pipx_sample = json!({
            "venvs": {
                "httpie": {
                    "metadata": {
                        "main_package": {
                            "package": "httpie",
                            "package_version": "3.2.2",
                            "apps": ["http", "https"]
                        }
                    }
                }
            }
        });

        let mut pkgs = Vec::new();
        let mut cmds = Vec::new();
        parse_pipx_json(&pipx_sample, &mut pkgs, &mut cmds);

        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].name, "httpie");
        assert_eq!(pkgs[0].version.as_deref(), Some("3.2.2"));
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "http");
        assert_eq!(cmds[1].name, "https");
    }

    #[test]
    fn test_parse_uv_tool_list() {
        let output = "ruff v0.4.1\n- ruff\nblack v24.4.2\n- black\n- blackd\n";
        let mut pkgs = Vec::new();
        let mut cmds = Vec::new();
        parse_uv_tool_list(output, &mut pkgs, &mut cmds);

        assert_eq!(pkgs.len(), 2);
        assert_eq!(cmds.len(), 3);
        assert_eq!(pkgs[0].name, "ruff");
        assert_eq!(pkgs[1].name, "black");
    }
}
