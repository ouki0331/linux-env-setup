//! Command-line argument parsing and command dispatch.

use crate::aggregate::aggregate_inventory;
use crate::aiexport::{generate_ai_input, generate_changes_only_export};
use crate::catalog::CatalogEntry;
use crate::config::Config;
use crate::diff::diff_inventories;
use crate::error::{Error, Result};
use crate::inventory::Inventory;
use crate::paths::{tilde, Paths};
use crate::release::{version_info, VERSION};
use crate::schema::validate_and_parse_catalog_json;
use crate::snapshot::{
    get_recent_complete_snapshots, get_snapshot, list_snapshots, prune_snapshots, save_snapshot,
};
use crate::sources::{all_adapters, ScanContext};
use crate::storage::{
    delete_entry, get_entry, import_entries_tx, list_entries, open_db, save_entry, search_entries,
};
use crate::util::{generate_snapshot_id, normalize_command_name};
use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "hdesk",
    about = "Personal CLI Help Desk: discover your global CLI tools and query a local command catalog",
    version = VERSION
)]
pub struct Cli {
    #[arg(
        value_name = "COMMAND",
        help = "Direct query for a command cheat sheet"
    )]
    pub direct_command: Option<String>,

    #[command(subcommand)]
    pub subcommand: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Display detailed cheat sheet card for a command")]
    Show { command: String },

    #[command(about = "List documented commands in the catalog")]
    List {
        #[arg(long, help = "Filter by kind (e.g. alias, rust_binary, python_cli)")]
        kind: Option<String>,

        #[arg(long, help = "Filter by source ecosystem (e.g. cargo, npm, zsh)")]
        source: Option<String>,
    },

    #[command(about = "Search catalog by keyword across name, summary, tags and syntax")]
    Search { keyword: String },

    #[command(about = "Manually add a new command entry")]
    Add { command: String },

    #[command(about = "Edit an existing command entry")]
    Edit { command: String },

    #[command(about = "Remove a command entry from the local catalog")]
    Remove {
        command: String,
        #[arg(short, long, help = "Confirm deletion without interactive prompt")]
        yes: bool,
    },

    #[command(about = "Scan local environment for installed CLI tools and shell definitions")]
    Scan {
        #[arg(short, long, help = "Write inventory JSON to file")]
        output: Option<PathBuf>,

        #[arg(long, help = "Save an immutable snapshot in the local database")]
        snapshot: bool,

        #[arg(
            long,
            help = "Allow sourcing a specific Zsh file in an isolated subshell"
        )]
        allow_source: Option<PathBuf>,
    },

    #[command(about = "Manage immutable environment snapshots")]
    Snapshot {
        #[command(subcommand)]
        cmd: SnapshotCommands,
    },

    #[command(about = "Compare two snapshots or review pending changes")]
    Diff {
        #[arg(
            long,
            help = "From snapshot ID (defaults to previous complete snapshot)"
        )]
        from: Option<String>,

        #[arg(long, help = "To snapshot ID (defaults to latest complete snapshot)")]
        to: Option<String>,
    },

    #[command(about = "Output shell integration script for active Zsh session")]
    ShellInit {
        #[arg(help = "Shell name (currently: zsh)")]
        shell: String,
    },

    #[command(about = "Inspect and summarize a local inventory JSON file")]
    Inventory { file: PathBuf },

    #[command(about = "Export redacted JSON for external AI completion")]
    ExportAi {
        #[arg(help = "Source inventory JSON file (optional if --changes-only is used)")]
        inventory_file: Option<PathBuf>,

        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Export only additions and items needing review between snapshots"
        )]
        changes_only: bool,

        #[arg(long, help = "Base snapshot ID for --changes-only")]
        from: Option<String>,

        #[arg(long, help = "Target snapshot ID for --changes-only")]
        to: Option<String>,
    },

    #[command(about = "Validate enriched catalog JSON against schema and local inventory")]
    Validate { file: PathBuf },

    #[command(about = "Transactionally import validated catalog JSON into local database")]
    Import {
        file: PathBuf,

        #[arg(
            long,
            help = "Restrict import strictly to targets listed in changes-only batch"
        )]
        changes_only: bool,
    },

    #[command(about = "Verify environment, storage paths, and toolchain health")]
    Doctor,
}

#[derive(Subcommand, Debug)]
pub enum SnapshotCommands {
    #[command(about = "List all stored snapshots")]
    List,

    #[command(about = "Prune older snapshots while keeping the newest N")]
    Prune {
        #[arg(long, default_value = "5", help = "Number of newest snapshots to keep")]
        keep: usize,

        #[arg(short, long, help = "Confirm pruning without interactive prompt")]
        yes: bool,
    },
}

pub fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::from(err.exit_code())
        }
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::from_env()?;

    if let Some(cmd) = &cli.direct_command {
        return cmd_show(&paths, cmd);
    }

    match cli.subcommand {
        Some(Commands::Show { command }) => cmd_show(&paths, &command),
        Some(Commands::List { kind, source }) => {
            cmd_list(&paths, kind.as_deref(), source.as_deref())
        }
        Some(Commands::Search { keyword }) => cmd_search(&paths, &keyword),
        Some(Commands::Add { command }) => cmd_add(&paths, &command),
        Some(Commands::Edit { command }) => cmd_edit(&paths, &command),
        Some(Commands::Remove { command, yes }) => cmd_remove(&paths, &command, yes),
        Some(Commands::Scan {
            output,
            snapshot,
            allow_source,
        }) => cmd_scan(&paths, output.as_deref(), snapshot, allow_source),
        Some(Commands::Snapshot { cmd }) => match cmd {
            SnapshotCommands::List => cmd_snapshot_list(&paths),
            SnapshotCommands::Prune { keep, yes } => cmd_snapshot_prune(&paths, keep, yes),
        },
        Some(Commands::Diff { from, to }) => cmd_diff(&paths, from.as_deref(), to.as_deref()),
        Some(Commands::ShellInit { shell }) => cmd_shell_init(&shell),
        Some(Commands::Inventory { file }) => cmd_inventory_inspect(&file),
        Some(Commands::ExportAi {
            inventory_file,
            output,
            changes_only,
            from,
            to,
        }) => cmd_export_ai(
            &paths,
            inventory_file.as_deref(),
            output.as_deref(),
            changes_only,
            from.as_deref(),
            to.as_deref(),
        ),
        Some(Commands::Validate { file }) => cmd_validate(&paths, &file),
        Some(Commands::Import { file, changes_only }) => cmd_import(&paths, &file, changes_only),
        Some(Commands::Doctor) => cmd_doctor(&paths),
        None => {
            println!("{}", version_info());
            println!("Run 'hdesk --help' for usage instructions.");
            Ok(())
        }
    }
}

fn cmd_show(paths: &Paths, command: &str) -> Result<()> {
    let conn = open_db(paths)?;
    let norm = normalize_command_name(command);
    let entry = get_entry(&conn, &norm)?;

    match entry {
        Some(e) => {
            print_command_card(&e, &paths.home);
            Ok(())
        }
        None => {
            Err(Error::NotFound(format!("Command '{command}' not found in catalog. Run 'hdesk search {command}' or 'hdesk scan'.")))
        }
    }
}

fn print_command_card(entry: &CatalogEntry, home: &Path) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  📖 {}  [{}]", entry.name, entry.kind);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if !entry.summary.is_empty() {
        println!("\n  Summary:\n    {}", entry.summary);
    }

    if !entry.syntax.is_empty() {
        println!("\n  Syntax:\n    $ {}", entry.syntax);
    }

    if let Some(p) = &entry.provider {
        let v_str = p.version.as_deref().unwrap_or("unknown");
        println!(
            "\n  Provider:\n    {} ({}) v{}",
            p.package, p.ecosystem, v_str
        );
    }

    if let Some(src) = &entry.source {
        if let Some(p) = &src.executable_path {
            println!("\n  Executable:\n    {}", tilde(Path::new(p), home));
        }
        if let Some(d) = &src.definition {
            println!("\n  Definition:\n    {}", d);
        }
    }

    if !entry.examples.is_empty() {
        println!("\n  Practical Examples:");
        for ex in &entry.examples {
            println!("    • {}", ex.description);
            println!("      $ {}", ex.command);
        }
    }

    if !entry.tags.is_empty() {
        println!("\n  Tags: [{}]", entry.tags.join(", "));
    }

    if entry.review_state != crate::catalog::ReviewState::Documented {
        println!("\n  Status: [{:?}]", entry.review_state);
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}

fn cmd_list(paths: &Paths, kind: Option<&str>, source: Option<&str>) -> Result<()> {
    let conn = open_db(paths)?;
    let entries = list_entries(&conn, kind, source)?;

    if entries.is_empty() {
        println!("Catalog is empty. Run 'hdesk scan' to discover your commands.");
        return Ok(());
    }

    println!("Documented Commands ({}):", entries.len());
    for e in entries {
        let eco = e
            .provider
            .as_ref()
            .map(|p| p.ecosystem.as_str())
            .unwrap_or("local");
        let sum = if e.summary.is_empty() {
            "(no summary)"
        } else {
            &e.summary
        };
        println!("  {:<16} [{:<12}] {:<10} : {}", e.name, e.kind, eco, sum);
    }
    Ok(())
}

fn cmd_search(paths: &Paths, keyword: &str) -> Result<()> {
    let conn = open_db(paths)?;
    let entries = search_entries(&conn, keyword)?;

    if entries.is_empty() {
        println!("No catalog entries matched keyword '{keyword}'.");
        return Ok(());
    }

    println!("Search Matches for '{}' ({}):", keyword, entries.len());
    for e in entries {
        println!("  {:<16} [{:<12}] : {}", e.name, e.kind, e.summary);
    }
    Ok(())
}

fn cmd_add(paths: &Paths, command: &str) -> Result<()> {
    let conn = open_db(paths)?;
    let norm = normalize_command_name(command);

    if get_entry(&conn, &norm)?.is_some() {
        return Err(Error::Usage(format!(
            "Command '{norm}' already exists. Use 'hdesk edit {norm}' to modify."
        )));
    }

    println!("Adding new command: {norm}");
    print!("Summary: ");
    io::stdout().flush().ok();
    let mut summary = String::new();
    io::stdin().read_line(&mut summary).ok();

    print!("Syntax ($): ");
    io::stdout().flush().ok();
    let mut syntax = String::new();
    io::stdin().read_line(&mut syntax).ok();

    let mut entry = CatalogEntry::new(format!("user:{norm}"), &norm, "user_command");
    entry.summary = summary.trim().to_string();
    entry.syntax = syntax.trim().to_string();

    save_entry(&conn, &entry)?;
    println!("✅ Added '{}' to catalog.", entry.name);
    Ok(())
}

fn cmd_edit(paths: &Paths, command: &str) -> Result<()> {
    let conn = open_db(paths)?;
    let norm = normalize_command_name(command);

    let mut entry = match get_entry(&conn, &norm)? {
        Some(e) => e,
        None => return Err(Error::NotFound(format!("Command '{norm}' not found."))),
    };

    println!("Editing '{}' (leave blank to keep current):", entry.name);
    println!("Current summary: {}", entry.summary);
    print!("New summary: ");
    io::stdout().flush().ok();
    let mut summary = String::new();
    io::stdin().read_line(&mut summary).ok();
    if !summary.trim().is_empty() {
        entry.summary = summary.trim().to_string();
    }

    println!("Current syntax: {}", entry.syntax);
    print!("New syntax: ");
    io::stdout().flush().ok();
    let mut syntax = String::new();
    io::stdin().read_line(&mut syntax).ok();
    if !syntax.trim().is_empty() {
        entry.syntax = syntax.trim().to_string();
    }

    entry.updated_at = crate::util::now_iso();
    save_entry(&conn, &entry)?;
    println!("✅ Updated '{}'.", entry.name);
    Ok(())
}

fn cmd_remove(paths: &Paths, command: &str, yes: bool) -> Result<()> {
    let conn = open_db(paths)?;
    let norm = normalize_command_name(command);

    let entry = match get_entry(&conn, &norm)? {
        Some(e) => e,
        None => {
            return Err(Error::NotFound(format!(
                "Command '{norm}' not found in catalog."
            )))
        }
    };

    if !yes {
        println!(
            "⚠️ Are you sure you want to remove catalog entry '{}' ({})? [y/N]",
            entry.name, entry.command_id
        );
        println!("Note: This only deletes the hdesk catalog card; no system binary or package will be removed.");
        print!("> ");
        io::stdout().flush().ok();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).ok();
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    delete_entry(&conn, &entry.command_id)?;
    println!("✅ Removed '{}' from catalog.", entry.name);
    Ok(())
}

fn cmd_scan(
    paths: &Paths,
    output: Option<&Path>,
    snapshot: bool,
    allow_source: Option<PathBuf>,
) -> Result<()> {
    let config = Config::load(paths)?;
    println!("🚀 Scanning personal CLI tools and shell definitions...");

    let ctx = ScanContext {
        paths,
        config: &config,
        allow_source,
        zsh_stdin: None,
        timeout: Duration::from_secs(8),
    };

    let mut all_packages = Vec::new();
    let mut all_commands = Vec::new();
    let mut reports = Vec::new();
    let mut all_ok = true;

    for adapter in all_adapters() {
        if !config.is_source_enabled(adapter.name()) {
            continue;
        }

        match adapter.scan(&ctx) {
            Ok(res) => {
                println!(
                    "  [adapter: {:<8}] status: {:<7} | pkgs: {:<3} | cmds: {:<3}",
                    res.name,
                    res.status.as_str(),
                    res.packages.len(),
                    res.commands.len()
                );
                if res.status == crate::sources::AdapterStatus::Failed {
                    all_ok = false;
                }
                reports.push(crate::inventory::AdapterReport {
                    name: res.name.to_string(),
                    status: res.status.as_str().to_string(),
                    packages_found: res.packages.len(),
                    commands_found: res.commands.len(),
                    error: res.error,
                });
                all_packages.extend(res.packages);
                all_commands.extend(res.commands);
            }
            Err(e) => {
                eprintln!("  [adapter: {:<8}] error: {e}", adapter.name());
                all_ok = false;
                reports.push(crate::inventory::AdapterReport {
                    name: adapter.name().to_string(),
                    status: "failed".to_string(),
                    packages_found: 0,
                    commands_found: 0,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let snapshot_id =
        generate_snapshot_id(&ctx.paths.home.to_string_lossy(), std::env::consts::ARCH);
    let mut inv = aggregate_inventory(all_packages, all_commands, snapshot_id, all_ok);
    inv.adapters = reports;

    println!(
        "\nDiscovered: {} packages, {} commands (complete: {})",
        inv.packages.len(),
        inv.commands.len(),
        inv.complete
    );

    if snapshot {
        let conn = open_db(paths)?;
        save_snapshot(&conn, &inv)?;
        println!("💾 Saved snapshot: {}", inv.snapshot_id);
    }

    if let Some(out_path) = output {
        let json_str = serde_json::to_string_pretty(&inv)?;
        fs::write(out_path, json_str)
            .map_err(|e| Error::io(format!("Writing inventory to {}", out_path.display()), e))?;
        println!("📄 Inventory written to: {}", out_path.display());
    }

    Ok(())
}

fn cmd_snapshot_list(paths: &Paths) -> Result<()> {
    let conn = open_db(paths)?;
    let snapshots = list_snapshots(&conn)?;

    if snapshots.is_empty() {
        println!("No snapshots stored. Run 'hdesk scan --snapshot' to create a baseline.");
        return Ok(());
    }

    println!("Stored Snapshots ({}):", snapshots.len());
    for s in snapshots {
        let state = if s.complete { "complete" } else { "partial" };
        println!(
            "  {:<36} | {:<20} | {:<8} | pkgs: {:<3} | cmds: {:<3}",
            s.snapshot_id, s.generated_at, state, s.package_count, s.command_count
        );
    }
    Ok(())
}

fn cmd_snapshot_prune(paths: &Paths, keep: usize, yes: bool) -> Result<()> {
    let conn = open_db(paths)?;
    let all = list_snapshots(&conn)?;

    if all.len() <= keep {
        println!(
            "Total snapshots ({}) does not exceed --keep ({}); nothing to prune.",
            all.len(),
            keep
        );
        return Ok(());
    }

    let count_to_delete = all.len() - keep;
    if !yes {
        println!(
            "⚠️ Prune {} older snapshot(s) keeping the newest {}? [y/N]",
            count_to_delete, keep
        );
        print!("> ");
        io::stdout().flush().ok();
        let mut ans = String::new();
        io::stdin().read_line(&mut ans).ok();
        if !ans.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let pruned = prune_snapshots(&conn, keep)?;
    println!("✅ Pruned {} snapshot(s).", pruned);
    Ok(())
}

fn cmd_diff(paths: &Paths, from_id: Option<&str>, to_id: Option<&str>) -> Result<()> {
    let conn = open_db(paths)?;

    let (from_inv, to_inv) = match (from_id, to_id) {
        (Some(f), Some(t)) => {
            let f_inv = get_snapshot(&conn, f)?
                .ok_or_else(|| Error::NotFound(format!("Snapshot '{f}' not found")))?;
            let t_inv = get_snapshot(&conn, t)?
                .ok_or_else(|| Error::NotFound(format!("Snapshot '{t}' not found")))?;
            (f_inv, t_inv)
        }
        _ => {
            let recent = get_recent_complete_snapshots(&conn, 2)?;
            if recent.len() < 2 {
                return Err(Error::Usage("Need at least 2 complete snapshots to compare. Run 'hdesk scan --snapshot' to create snapshots.".into()));
            }
            (recent[1].clone(), recent[0].clone())
        }
    };

    println!(
        "Comparing snapshot '{}' -> '{}':",
        from_inv.snapshot_id, to_inv.snapshot_id
    );
    let diff = diff_inventories(&from_inv, &to_inv);

    if diff.is_empty {
        println!("✨ No differences detected. Your CLI environment is unchanged.");
        return Ok(());
    }

    if !diff.packages.is_empty() {
        println!("\n📦 Package Changes ({}):", diff.packages.len());
        for p in diff.packages {
            println!(
                "  [{:<15}] {:<20} ({}) : {:?} -> {:?}",
                p.change.as_str(),
                p.name,
                p.ecosystem,
                p.old_version,
                p.new_version
            );
        }
    }

    if !diff.commands.is_empty() {
        println!("\n⚡ Command Changes ({}):", diff.commands.len());
        for c in diff.commands {
            println!(
                "  [{:<15}] {:<20} : {}",
                c.change.as_str(),
                c.name,
                c.reason
            );
        }
    }

    Ok(())
}

fn cmd_shell_init(shell: &str) -> Result<()> {
    if !shell.eq_ignore_ascii_case("zsh") {
        return Err(Error::Usage(format!(
            "Shell '{shell}' is not currently supported; only 'zsh' is supported."
        )));
    }
    println!("{}", crate::sources::zsh::generate_shell_init_script());
    Ok(())
}

fn cmd_inventory_inspect(file: &Path) -> Result<()> {
    let content = fs::read_to_string(file)
        .map_err(|e| Error::io(format!("Reading {}", file.display()), e))?;
    let inv: Inventory = serde_json::from_str(&content)
        .map_err(|e| Error::Json(format!("Parsing {}: {e}", file.display())))?;

    println!("Inventory: {}", file.display());
    println!("  Snapshot ID : {}", inv.snapshot_id);
    println!("  Generated At: {}", inv.generated_at);
    println!("  Complete    : {}", inv.complete);
    println!(
        "  Host        : {} ({}) / shell: {}",
        inv.host.os, inv.host.arch, inv.host.shell
    );
    println!("  Packages    : {}", inv.packages.len());
    println!("  Commands    : {}", inv.commands.len());

    if !inv.adapters.is_empty() {
        println!("  Adapters:");
        for a in &inv.adapters {
            println!(
                "    • {:<10} : {:<8} (pkgs: {}, cmds: {})",
                a.name, a.status, a.packages_found, a.commands_found
            );
        }
    }
    Ok(())
}

fn cmd_export_ai(
    paths: &Paths,
    inventory_file: Option<&Path>,
    output: Option<&Path>,
    changes_only: bool,
    from_id: Option<&str>,
    to_id: Option<&str>,
) -> Result<()> {
    let out_json = if changes_only {
        let conn = open_db(paths)?;
        let (from_inv, to_inv) = match (from_id, to_id) {
            (Some(f), Some(t)) => {
                let f_inv = get_snapshot(&conn, f)?
                    .ok_or_else(|| Error::NotFound(format!("Snapshot '{f}' not found")))?;
                let t_inv = get_snapshot(&conn, t)?
                    .ok_or_else(|| Error::NotFound(format!("Snapshot '{t}' not found")))?;
                (f_inv, t_inv)
            }
            _ => {
                let recent = get_recent_complete_snapshots(&conn, 2)?;
                if recent.len() < 2 {
                    return Err(Error::Usage("Need at least 2 complete snapshots to compute changes. Run 'hdesk scan --snapshot'.".into()));
                }
                (recent[1].clone(), recent[0].clone())
            }
        };

        let current_catalog = list_entries(&conn, None, None)?;
        let payload =
            generate_changes_only_export(&from_inv, &to_inv, &current_catalog, &paths.home)?;
        serde_json::to_string_pretty(&payload)?
    } else {
        let inv = match inventory_file {
            Some(f) => {
                let content = fs::read_to_string(f)
                    .map_err(|e| Error::io(format!("Reading {}", f.display()), e))?;
                serde_json::from_str::<Inventory>(&content)?
            }
            None => {
                let conn = open_db(paths)?;
                let recent = get_recent_complete_snapshots(&conn, 1)?;
                if recent.is_empty() {
                    return Err(Error::Usage("No inventory file specified and no snapshot found in DB. Run 'hdesk scan --output inventory.json'.".into()));
                }
                recent[0].clone()
            }
        };

        let payload = generate_ai_input(&inv, &paths.home);
        serde_json::to_string_pretty(&payload)?
    };

    match output {
        Some(p) => {
            fs::write(p, &out_json)
                .map_err(|e| Error::io(format!("Writing to {}", p.display()), e))?;
            println!("✅ AI prompt payload written to: {}", p.display());
        }
        None => {
            println!("{out_json}");
        }
    }

    Ok(())
}

fn cmd_validate(paths: &Paths, file: &Path) -> Result<()> {
    let content = fs::read_to_string(file)
        .map_err(|e| Error::io(format!("Reading {}", file.display()), e))?;

    // Attempt to gather known command IDs from snapshots to guard against hallucination
    let conn = open_db(paths).ok();
    let mut known_ids = HashSet::new();
    if let Some(c) = conn {
        if let Ok(snapshots) = get_recent_complete_snapshots(&c, 5) {
            for s in snapshots {
                for cmd in s.commands {
                    known_ids.insert(cmd.id);
                }
            }
        }
    }

    let allowed = if known_ids.is_empty() {
        None
    } else {
        Some(&known_ids)
    };
    let entries = validate_and_parse_catalog_json(&content, allowed)?;

    println!(
        "✅ Validation succeeded: {} valid command entry(ies) found in {}",
        entries.len(),
        file.display()
    );
    Ok(())
}

fn cmd_import(paths: &Paths, file: &Path, changes_only: bool) -> Result<()> {
    let content = fs::read_to_string(file)
        .map_err(|e| Error::io(format!("Reading {}", file.display()), e))?;
    let mut conn = open_db(paths)?;

    let mut known_ids = HashSet::new();
    if let Ok(snapshots) = get_recent_complete_snapshots(&conn, 5) {
        for s in snapshots {
            for cmd in s.commands {
                known_ids.insert(cmd.id);
            }
        }
    }

    let allowed = if known_ids.is_empty() {
        None
    } else {
        Some(&known_ids)
    };
    let entries = validate_and_parse_catalog_json(&content, allowed)?;

    let target_ids: Option<Vec<String>> = if changes_only {
        Some(entries.iter().map(|e| e.command_id.clone()).collect())
    } else {
        None
    };

    let imported = import_entries_tx(&mut conn, &entries, target_ids.as_deref())?;
    println!(
        "🎉 Transactional import successful: {} command(s) saved to catalog.",
        imported
    );
    Ok(())
}

fn cmd_doctor(paths: &Paths) -> Result<()> {
    println!("============================================================");
    println!("🩺 hdesk System Health Check");
    println!("============================================================");

    println!("Paths:");
    println!("  Data directory  : {}", paths.data_dir.display());
    println!("  Catalog DB path : {}", paths.db_path().display());
    println!("  Config path     : {}", paths.config_path().display());

    let db_ok = match open_db(paths) {
        Ok(_) => "accessible and writable",
        Err(_) => "inaccessible",
    };
    println!("  Database status : {db_ok}");

    let legacy_db = paths.legacy_db_path();
    if legacy_db.exists() {
        println!(
            "  Legacy database : {} (present; untouched by v0.1 as per spec)",
            legacy_db.display()
        );
    } else {
        println!("  Legacy database : not present");
    }

    println!("\nToolchain Availability:");
    let tools = ["cargo", "python3", "pipx", "uv", "npm", "composer", "zsh"];
    for t in tools {
        let status = if crate::exec::is_executable_available(t) {
            "available"
        } else {
            "not found"
        };
        println!("  {:<12} : {status}", t);
    }

    println!("============================================================");
    Ok(())
}
