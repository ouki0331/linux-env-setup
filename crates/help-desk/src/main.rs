mod app;
mod db;
mod models;
mod ui;

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::env;
use std::fs;
use std::io::{self, stdout};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let conn = db::get_connection()?;
    db::auto_seed_if_empty(&conn)?;

    // Handle CLI subcommands and non-interactive arguments
    if args.len() > 1 {
        let first_arg = &args[1];

        if first_arg == "--help" || first_arg == "-h" {
            print_help();
            return Ok(());
        }

        if first_arg == "--import" {
            if args.len() < 3 {
                eprintln!("Usage: help-desk --import <file.json>");
                std::process::exit(1);
            }
            let file_path = &args[2];
            match db::import_from_file(&conn, file_path) {
                Ok(n) => println!("✅ Successfully imported/updated {} command entries into Help Desk database.", n),
                Err(e) => {
                    eprintln!("❌ Import failed: {}", e);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        if first_arg == "--list" {
            let commands = db::load_all_commands(&conn);
            println!("Documented commands ({}):", commands.len());
            for c in commands {
                println!("  {:<12} [{}] : {}", c.command, c.category_ja, c.summary_ja);
            }
            return Ok(());
        }

        if first_arg == "--diff" {
            let undoc = get_undocumented_packages(&conn);
            if undoc.is_empty() {
                println!("All known packages are already documented.");
            } else {
                println!("Undocumented packages ({}):", undoc.len());
                for u in undoc {
                    println!("  • {}", u);
                }
            }
            return Ok(());
        }

        if first_arg == "--export-prompt" {
            let targets: Vec<String> = if args.len() > 2 {
                args[2..].to_vec()
            } else {
                let undoc = get_undocumented_packages(&conn);
                if undoc.is_empty() {
                    scan_packages_txt()
                } else {
                    undoc
                }
            };
            println!("{}", generate_ai_prompt(&targets));
            return Ok(());
        }

        // Direct command lookup (e.g. `help-desk tar` or `help-desk req`)
        if !first_arg.starts_with('-') {
            let commands = db::load_all_commands(&conn);
            if let Some(c) = commands.iter().find(|cmd| cmd.command.eq_ignore_ascii_case(first_arg)) {
                print_cli_card(c);
            } else {
                eprintln!("Command '{}' not found in Help Desk.", first_arg);
                let candidates: Vec<&str> = commands
                    .iter()
                    .filter(|cmd| cmd.command.contains(first_arg))
                    .map(|cmd| cmd.command.as_str())
                    .collect();
                if !candidates.is_empty() {
                    eprintln!("Did you mean: {}", candidates.join(", "));
                }
            }
            return Ok(());
        }
    }

    // Interactive TUI Mode
    let commands = db::load_all_commands(&conn);
    run_tui(commands)?;

    Ok(())
}

fn print_help() {
    println!(r#"Personal Linux Help Desk & Command Catalog (Rust TUI)

USAGE:
  help-desk                   Launch interactive split-panel TUI (fzf search & browsing)
  help-desk <command>         Display cheat card for specific command directly
  help-desk --import <file>   Import AI-generated JSON file into local SQLite
  help-desk --list            List all documented tools
  help-desk --diff            List packages from packages.txt missing documentation
  help-desk --export-prompt   Generate AI prompt with schema for incremental packages

KEYBINDINGS IN TUI:
  [Type letters]              Real-time fzf-like fuzzy search
  [Enter]                     Focus Detail Panel (enable scrolling)
  [Esc]                       Back to List / Clear search / Exit
  [Tab / F1 / Ctrl+S]         Switch Sort Mode (Name, Category, Date)
  [F3 / Ctrl+O]               Toggle Sort Order (Ascending / Descending)
  [F2 / Ctrl+L / l]           Toggle Language (Japanese 🇯🇵 / English 🇺🇸)
  [↑/↓, j/k, Mouse Wheel]     Navigate & Scroll
  [Ctrl+C / q]                Quit
"#);
}

fn print_cli_card(cmd: &models::CommandDoc) {
    println!("============================================================");
    println!("📖 [ヘルプデスク] {} ({})", cmd.command, cmd.category_ja);
    println!("============================================================");
    println!("【概要 / Summary】\n  {}", cmd.summary_ja);
    if !cmd.description_ja.is_empty() {
        println!("\n【詳細解説 / Description】\n  {}", cmd.description_ja);
    }
    if !cmd.syntax.is_empty() {
        println!("\n【基本構文 / Syntax】\n  $ {}", cmd.syntax);
    }
    if !cmd.options.is_empty() {
        println!("\n【主なオプション / Key Options】");
        for opt in &cmd.options {
            println!("  {:<16} : {}", opt.flag, opt.desc_ja);
        }
    }
    if !cmd.examples.is_empty() {
        println!("\n【よく使う実用例 / Everyday Examples】");
        for (i, ex) in cmd.examples.iter().enumerate() {
            println!("  {}. {}\n     ❯ {}", i + 1, ex.description_ja, ex.command);
        }
    }
    println!("============================================================");
}

fn scan_packages_txt() -> Vec<String> {
    let candidates = [
        "packages.txt",
        "../packages.txt",
        "../../packages.txt",
    ];
    for c in candidates {
        if let Ok(content) = fs::read_to_string(c) {
            let pkgs: Vec<String> = content
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|s| s.to_string())
                .collect();
            if !pkgs.is_empty() {
                return pkgs;
            }
        }
    }
    vec!["w3m".into(), "tar".into(), "curl".into(), "req".into()]
}

fn get_undocumented_packages(conn: &rusqlite::Connection) -> Vec<String> {
    let pkgs = scan_packages_txt();
    let commands = db::load_all_commands(conn);
    let documented: std::collections::HashSet<String> = commands.into_iter().map(|c| c.command).collect();
    pkgs.into_iter().filter(|p| !documented.contains(p)).collect()
}

fn generate_ai_prompt(pkgs: &[String]) -> String {
    let pkgs_str = pkgs.join(", ");
    format!(
r#"You are a professional Linux systems engineer and technical documentation author.
Please generate comprehensive, practical Japanese and English documentation for the following Linux commands / packages:
{}

### Output Instructions:
1. Provide the output strictly as a valid JSON array of objects.
2. Use natural, precise technical Japanese and English.
3. Each command object must strictly adhere to the following schema:

```json
[
  {{
    "command": "package_or_command_name",
    "category_ja": "One of: ファイル操作・圧縮, ネットワーク・通信, 自作開発ツール, システム監視・プロセス, 開発・エディタ, モダンCLI拡張",
    "category_en": "One of: File Ops & Archives, Network & HTTP, Custom Tools, System & Processes, Dev & Editors, Modern CLI",
    "summary_ja": "日本語の一行要約",
    "summary_en": "One-line summary in English",
    "description_ja": "日本語の詳細説明と使いどころ",
    "description_en": "Detailed explanation and key use-cases in English",
    "syntax": "command [options] [args]",
    "options": [
      {{ "flag": "-flag", "desc_ja": "オプション説明 (日本語)", "desc_en": "Option description (English)" }}
    ],
    "examples": [
      {{
        "description_ja": "実用シナリオ (日本語)",
        "description_en": "Practical scenario (English)",
        "command": "command --example-invocation"
      }}
    ]
  }}
]
```

Please generate the complete JSON array for all specified packages: {}."#,
        pkgs_str, pkgs_str
    )
}

fn run_tui(commands: Vec<models::CommandDoc>) -> Result<(), Box<dyn std::error::Error>> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Panic hook to ensure terminal restoration
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    let mut app = App::new(commands);

    // Main Event Loop
    while !app.should_quit {
        terminal.draw(|f| ui::render(&app, f))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_key_event(key);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse_event(mouse);
                }
                _ => {}
            }
        }
    }

    // Terminal teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
