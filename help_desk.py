#!/usr/bin/env python3
"""
Interactive CLI Help Desk & Command Catalog (help-desk / hdesk)
- Zero external dependencies: pure Python 3 standard library
- SQLite persistence: stores localized Japanese command references, options, and practical examples
- Multi-dimensional browsing: Browse by Category, Browse Alphabetically (A-Z), and Keyword Search
- Instant single-keypress navigation (1-9, 'b' to go back, Enter to select)
- Direct command lookup: `help-desk <command>` (e.g. `help-desk tar` or `hdesk req`)
- Incremental AI Prompt Generator: exports package diffs and schema ready for web AI (ChatGPT/Claude/Gemini)
- Seamless JSON Importer: refreshes local database with AI-generated JSON batches
"""

import sys
import os
import json
import sqlite3
import argparse
from pathlib import Path

# Terminal single-key helper
try:
    import termios
    import tty
    HAS_TERMIOS = True
except ImportError:
    HAS_TERMIOS = False

# ==========================================
# Terminal Colors & Styling
# ==========================================
class Color:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"

def cprint(text, color="", bold=False, end="\n"):
    b = Color.BOLD if bold else ""
    print(f"{b}{color}{text}{Color.RESET}", end=end)

# ==========================================
# Database Management (SQLite)
# ==========================================
DB_PATH = Path(os.environ.get("HELP_DESK_DB", str(Path.home() / ".help_desk.db")))

def get_db():
    global DB_PATH
    try:
        conn = sqlite3.connect(str(DB_PATH))
        conn.row_factory = sqlite3.Row
        return conn
    except sqlite3.OperationalError:
        fallback = Path.cwd() / ".help_desk.db"
        DB_PATH = fallback
        conn = sqlite3.connect(str(DB_PATH))
        conn.row_factory = sqlite3.Row
        return conn

def init_db():
    conn = get_db()
    with conn:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS commands (
                command TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                summary_ja TEXT NOT NULL,
                description_ja TEXT NOT NULL,
                syntax TEXT NOT NULL,
                options_json TEXT,
                examples_json TEXT,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS package_snapshots (
                package_name TEXT PRIMARY KEY,
                version TEXT,
                source TEXT,
                recorded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
    conn.close()

def auto_seed_if_empty():
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT COUNT(*) as cnt FROM commands")
    row = cur.fetchone()
    count = row['cnt'] if row else 0
    conn.close()

    if count == 0:
        script_dir = Path(__file__).resolve().parent
        seed_file = script_dir / "help_data_ja.json"
        if seed_file.is_file():
            import_json_data(str(seed_file), silent=True)

def import_json_data(json_file_path, silent=False):
    p = Path(json_file_path).expanduser().resolve()
    if not p.is_file():
        if not silent:
            cprint(f"❌ File not found: {json_file_path}", Color.RED)
        return 0

    with open(p, "r", encoding="utf-8") as f:
        data = json.load(f)

    if not isinstance(data, list):
        if not silent:
            cprint("❌ Invalid format: Root JSON must be an array of command objects.", Color.RED)
        return 0

    conn = get_db()
    count = 0
    with conn:
        for item in data:
            cmd = item.get("command", "").strip()
            cat = item.get("category", "General").strip()
            summary = item.get("summary_ja", "").strip()
            desc = item.get("description_ja", "").strip()
            syntax = item.get("syntax", "").strip()
            options = json.dumps(item.get("options", []), ensure_ascii=False)
            examples = json.dumps(item.get("examples", []), ensure_ascii=False)

            if cmd:
                conn.execute("""
                    INSERT INTO commands (command, category, summary_ja, description_ja, syntax, options_json, examples_json, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                    ON CONFLICT(command) DO UPDATE SET
                        category = excluded.category,
                        summary_ja = excluded.summary_ja,
                        description_ja = excluded.description_ja,
                        syntax = excluded.syntax,
                        options_json = excluded.options_json,
                        examples_json = excluded.examples_json,
                        updated_at = CURRENT_TIMESTAMP
                """, (cmd, cat, summary, desc, syntax, options, examples))
                count += 1
    conn.close()
    if not silent:
        cprint(f"✅ Successfully imported/updated {count} command entries into Help Desk database.", Color.GREEN, bold=True)
    return count

def get_command(command_name):
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT * FROM commands WHERE command = ?", (command_name.strip(),))
    row = cur.fetchone()
    conn.close()
    return row

def get_all_commands():
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT command, category, summary_ja FROM commands ORDER BY command ASC")
    rows = cur.fetchall()
    conn.close()
    return rows

def get_categories():
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT category, COUNT(*) as cnt FROM commands GROUP BY category ORDER BY cnt DESC, category ASC")
    rows = cur.fetchall()
    conn.close()
    return rows

def get_commands_by_category(category_name):
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT command, summary_ja FROM commands WHERE category = ? ORDER BY command ASC", (category_name,))
    rows = cur.fetchall()
    conn.close()
    return rows

def search_commands(keyword):
    conn = get_db()
    cur = conn.cursor()
    kw = f"%{keyword.strip()}%"
    cur.execute("""
        SELECT command, category, summary_ja FROM commands
        WHERE command LIKE ? OR summary_ja LIKE ? OR description_ja LIKE ? OR options_json LIKE ?
        ORDER BY command ASC
    """, (kw, kw, kw, kw))
    rows = cur.fetchall()
    conn.close()
    return rows

# ==========================================
# Terminal Single-Key Helpers
# ==========================================
def get_single_char():
    if not HAS_TERMIOS or not sys.stdin.isatty():
        line = sys.stdin.readline()
        return line[0] if line else ""
    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    try:
        tty.setcbreak(fd)
        ch = sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old)
    return ch

def prompt_choice(title, options, default_idx=0, allow_back=True):
    cprint(f"\n? {title}", Color.CYAN, bold=True)
    for i, (label, _) in enumerate(options, 1):
        if i - 1 == default_idx:
            cprint(f"  [{i}] {label} (default)", Color.GREEN, bold=True)
        else:
            cprint(f"  [{i}] {label}", Color.WHITE)
            
    if allow_back:
        cprint(f"  [b] < Back / 戻る", Color.YELLOW)

    use_single_key = (len(options) <= 9)
    allowed_digits = [str(i) for i in range(1, len(options) + 1)]
    back_tip = ", 'b' to go back" if allow_back else ""
    cprint(f"  ℹ️ Tip: Press 1-{len(options)} directly (no Enter needed){back_tip}, or Enter for default [{default_idx+1}].", Color.DIM)
    sys.stdout.write(f"❯ ")
    sys.stdout.flush()

    if use_single_key and HAS_TERMIOS and sys.stdin.isatty():
        while True:
            ch = get_single_char()
            if ch in ['\r', '\n']:
                val = options[default_idx][1]
                print(f"{Color.GREEN}{default_idx+1} (default){Color.RESET}")
                return val
            ch_lower = ch.lower()
            if allow_back and ch_lower == 'b':
                print(f"{Color.YELLOW}b (Back){Color.RESET}")
                return "__BACK__"
            if ch_lower in allowed_digits:
                idx = int(ch_lower) - 1
                val = options[idx][1]
                print(f"{Color.GREEN}{ch_lower}{Color.RESET}")
                return val
    else:
        while True:
            choice = input().strip().lower()
            if not choice:
                return options[default_idx][1]
            if allow_back and choice in ["b", "back"]:
                return "__BACK__"
            if choice.isdigit():
                idx = int(choice) - 1
                if 0 <= idx < len(options):
                    return options[idx][1]
            cprint("Invalid selection. Try again.", Color.RED)
            sys.stdout.write(f"❯ ")
            sys.stdout.flush()

def prompt_input(title, default="", allow_back=True):
    default_hint = f" [default: {default}]" if default else ""
    cprint(f"\n? {title}{default_hint}", Color.CYAN, bold=True)
    back_tip = " (or type 'b' to go back)" if allow_back else ""
    cprint(f"  ℹ️ Tip: Press Enter directly to accept default{back_tip}.", Color.DIM)
    val = input("❯ ").strip()
    if allow_back and val.lower() == "b":
        return "__BACK__"
    return val if val else default

# ==========================================
# Cheat Card View
# ==========================================
def display_command_card(row):
    if not row:
        cprint("⚠️ Command not found in Help Desk.", Color.YELLOW)
        return

    cmd = row['command']
    cat = row['category']
    summary = row['summary_ja']
    desc = row['description_ja']
    syntax = row['syntax']
    options = json.loads(row['options_json']) if row['options_json'] else []
    examples = json.loads(row['examples_json']) if row['examples_json'] else []

    print("\n" + "="*60)
    cprint(f"📖 [ヘルプデスク] {cmd} ({cat})", Color.CYAN, bold=True)
    print("="*60)

    cprint("【概要 / Summary】", Color.MAGENTA, bold=True)
    cprint(f"  {summary}", Color.WHITE)

    if desc:
        cprint("\n【詳細解説 / Description】", Color.MAGENTA, bold=True)
        cprint(f"  {desc}", Color.DIM)

    if syntax:
        cprint("\n【基本構文 / Syntax】", Color.YELLOW, bold=True)
        cprint(f"  $ {syntax}", Color.GREEN)

    if options:
        cprint("\n【主なオプション / Key Options】", Color.YELLOW, bold=True)
        for opt in options:
            flag = opt.get("flag", "")
            d = opt.get("desc_ja", "")
            cprint(f"  {Color.BOLD}{flag:<16}{Color.RESET} : {d}", Color.WHITE)

    if examples:
        cprint("\n【よく使う実用例 / Everyday Examples】", Color.GREEN, bold=True)
        for i, ex in enumerate(examples, 1):
            ex_desc = ex.get("description_ja", "")
            ex_cmd = ex.get("command", "")
            cprint(f"  {i}. {ex_desc}", Color.CYAN)
            cprint(f"     ❯ {ex_cmd}", Color.BOLD + Color.WHITE)
            print()

    print("="*60)

# ==========================================
# Interactive Navigation Flows
# ==========================================
def browse_by_category_flow():
    while True:
        cats = get_categories()
        if not cats:
            cprint("No categories found in Help Desk database.", Color.YELLOW)
            return

        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint("📁 Browse by Category (カテゴリ別一覧)", Color.BLUE, bold=True)
        cprint("="*50, Color.BLUE, bold=True)

        options = [(f"{r['category']} ({r['cnt']} tools)", r['category']) for r in cats]
        options.append(("Back to Main Menu", "__BACK__"))

        selected_cat = prompt_choice("Select Category:", options, default_idx=0, allow_back=False)
        if selected_cat == "__BACK__":
            break

        # List commands in category
        while True:
            cmd_rows = get_commands_by_category(selected_cat)
            cprint("\n" + "-"*50, Color.CYAN)
            cprint(f"📂 Category: [{selected_cat}] ({len(cmd_rows)} commands)", Color.CYAN, bold=True)
            cprint("-"*50, Color.CYAN)

            sub_opts = [(f"{r['command']:<12} - {r['summary_ja']}", r['command']) for r in cmd_rows]
            sub_opts.append(("Back to Categories", "__BACK__"))

            chosen_cmd = prompt_choice("Select Command to View Details:", sub_opts, default_idx=0, allow_back=False)
            if chosen_cmd == "__BACK__":
                break

            card_row = get_command(chosen_cmd)
            display_command_card(card_row)

            # Wait for key before going back
            cprint("Press any key to return to command list...", Color.DIM)
            get_single_char()

def browse_alphabetical_flow():
    while True:
        all_cmds = get_all_commands()
        if not all_cmds:
            cprint("No commands found in database.", Color.YELLOW)
            return

        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint(f"🔤 Browse A-Z (全コマンド一覧: {len(all_cmds)} tools)", Color.BLUE, bold=True)
        cprint("="*50, Color.BLUE, bold=True)

        # Show in chunks of 8 with pagination if needed
        options = [(f"{r['command']:<10} [{r['category']}] - {r['summary_ja'][:35]}", r['command']) for r in all_cmds]
        options.append(("Back to Main Menu", "__BACK__"))

        chosen_cmd = prompt_choice("Select Command to Inspect:", options, default_idx=0, allow_back=False)
        if chosen_cmd == "__BACK__":
            break

        card_row = get_command(chosen_cmd)
        display_command_card(card_row)

        cprint("Press any key to return to command list...", Color.DIM)
        get_single_char()

def search_flow():
    while True:
        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint("🔍 Keyword Search (コマンド検索)", Color.BLUE, bold=True)
        cprint("="*50, Color.BLUE, bold=True)

        kw = prompt_input("Enter search keyword (command, option, or Japanese term):")
        if kw == "__BACK__" or not kw:
            break

        results = search_commands(kw)
        if not results:
            cprint(f"No commands matching '{kw}' found.", Color.YELLOW)
            continue

        cprint(f"\nFound {len(results)} matches for '{kw}':", Color.GREEN, bold=True)
        options = [(f"{r['command']:<10} [{r['category']}] - {r['summary_ja'][:40]}", r['command']) for r in results]
        options.append(("Search again", "__AGAIN__"))
        options.append(("Back to Main Menu", "__BACK__"))

        chosen = prompt_choice("Select Result to Inspect:", options, default_idx=0, allow_back=False)
        if chosen == "__BACK__":
            break
        elif chosen == "__AGAIN__":
            continue
        else:
            card_row = get_command(chosen)
            display_command_card(card_row)
            cprint("Press any key to return...", Color.DIM)
            get_single_char()

# ==========================================
# Incremental Package Scanner & AI Prompt Builder
# ==========================================
def scan_system_packages():
    """Scan packages.txt and custom repository scripts."""
    packages = []
    script_dir = Path(__file__).resolve().parent
    repo_dir = script_dir

    # 1. packages.txt
    pkg_file = repo_dir / "packages.txt"
    if pkg_file.is_file():
        with open(pkg_file, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#"):
                    packages.append(line)

    # 2. Custom tools
    custom_tools = ["req", "deploy.py", "rollback.py", "backup.sh", "restore.sh"]
    for t in custom_tools:
        if (repo_dir / t).exists() or (repo_dir / f"{t}.py").exists() or t in ["req"]:
            if t not in packages:
                packages.append(t)

    return sorted(list(set(packages)))

def get_undocumented_packages(packages_list=None):
    """Find packages that are not yet documented in the Help Desk database."""
    if packages_list is None:
        packages_list = scan_system_packages()

    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT command FROM commands")
    documented = {r['command'] for r in cur.fetchall()}
    conn.close()

    undocumented = [pkg for pkg in packages_list if pkg not in documented]
    return undocumented

def generate_ai_prompt(target_packages):
    """Generate structured prompt with JSON schema for web AI (ChatGPT/Claude/Gemini)."""
    pkgs_str = ", ".join(target_packages)

    prompt = f"""You are a professional Linux systems engineer and technical documentation author.
Please generate comprehensive, practical Japanese documentation for the following Linux commands / packages:
{pkgs_str}

### Output Instructions:
1. Provide the output strictly as a valid JSON array of objects.
2. Do not wrap in markdown quotes if possible, or wrap in a single ```json codeblock.
3. Use natural, precise, and practical technical Japanese (自然で実用的な日本語).
4. Each command object must strictly adhere to the following schema:

```json
[
  {{
    "command": "package_or_command_name",
    "category": "One of: ファイル操作・圧縮, ネットワーク・通信, 自作開発ツール, システム監視・プロセス, 開発・エディタ, モダンCLI拡張",
    "summary_ja": "One-line clear summary in Japanese explaining the tool's primary purpose",
    "description_ja": "Detailed technical explanation in Japanese detailing what problem it solves and its key capabilities",
    "syntax": "Standard command syntax (e.g. tar [options] [archive] [target])",
    "options": [
      {{ "flag": "-flag", "desc_ja": "Explanation of the flag in Japanese" }}
    ],
    "examples": [
      {{
        "description_ja": "Practical everyday use-case description in Japanese",
        "command": "Exact command invocation"
      }}
    ]
  }}
]
```

Please generate the complete JSON array for all specified packages: {pkgs_str}.
"""
    return prompt

def export_ai_prompt_flow():
    cprint("\n" + "="*50, Color.BLUE, bold=True)
    cprint("🤖 Export AI Prompt for Documentation (AIプロンプト生成)", Color.BLUE, bold=True)
    cprint("="*50, Color.BLUE, bold=True)

    undoc = get_undocumented_packages()
    all_known = scan_system_packages()

    cprint(f"Total known packages/tools : {len(all_known)}", Color.WHITE)
    cprint(f"Undocumented (incremental) : {len(undoc)}", Color.YELLOW if undoc else Color.GREEN)

    opts = [
        (f"Export Incremental packages only ({len(undoc)} tools)", "incremental"),
        (f"Export All packages ({len(all_known)} tools)", "all"),
        ("Custom input of package names", "custom"),
        ("Back to Main Menu", "__BACK__")
    ]

    choice = prompt_choice("Select Export Scope:", opts, default_idx=0, allow_back=False)
    if choice == "__BACK__":
        return

    target_list = []
    if choice == "incremental":
        if not undoc:
            cprint("🎉 All known packages are already documented in Help Desk!", Color.GREEN, bold=True)
            return
        target_list = undoc
    elif choice == "all":
        target_list = all_known
    elif choice == "custom":
        inp = prompt_input("Enter package names separated by commas (e.g. nginx, docker, rsync):")
        if inp == "__BACK__" or not inp:
            return
        target_list = [x.strip() for x in inp.split(",") if x.strip()]

    prompt_text = generate_ai_prompt(target_list)

    # Save prompt to file
    out_file = Path.cwd() / "help_desk_ai_prompt.txt"
    with open(out_file, "w", encoding="utf-8") as f:
        f.write(prompt_text)

    print("\n" + "="*60)
    cprint(f"📋 AI Prompt Generated for [{len(target_list)}] packages:", Color.GREEN, bold=True)
    cprint(f"💾 Saved to: {out_file}", Color.CYAN)
    print("="*60)
    cprint(prompt_text, Color.WHITE)
    print("="*60)
    cprint("👉 Next Steps:", Color.YELLOW, bold=True)
    cprint("1. Copy the prompt above and paste it into ChatGPT / Claude / Gemini web interface.", Color.WHITE)
    cprint("2. Save the AI output into a JSON file (e.g. 'help_update.json').", Color.WHITE)
    cprint("3. In Help Desk, select '[5] Import AI-Generated JSON' or run 'help-desk --import help_update.json'.", Color.WHITE)

# ==========================================
# Main Menu & CLI Entrypoint
# ==========================================
def main_interactive():
    init_db()
    auto_seed_if_empty()

    while True:
        all_cmds = get_all_commands()
        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint("📖 Personal Linux Help Desk & Catalog (help-desk)", Color.BLUE, bold=True)
        cprint(f"📦 Total Documented Tools: {len(all_cmds)} | Storage: SQLite ({DB_PATH})", Color.DIM)
        cprint("="*50, Color.BLUE, bold=True)

        menu_opts = [
            ("Browse by Category (カテゴリ別一覧)", "cat"),
            ("Browse Alphabetically A-Z (全コマンド一覧)", "az"),
            ("Keyword Search (キーワード検索)", "search"),
            ("Export AI Prompt for Packages (プロンプト生成)", "export"),
            ("Import AI-Generated JSON (インポート・反映)", "import"),
            ("Exit", "exit")
        ]

        choice = prompt_choice("Select Action:", menu_opts, default_idx=0, allow_back=False)
        if choice == "cat":
            browse_by_category_flow()
        elif choice == "az":
            browse_alphabetical_flow()
        elif choice == "search":
            search_flow()
        elif choice == "export":
            export_ai_prompt_flow()
        elif choice == "import":
            path_str = prompt_input("Enter path to JSON file to import:", default="help_data_ja.json")
            if path_str != "__BACK__" and path_str:
                import_json_data(path_str)
        elif choice == "exit":
            cprint("\n👋 Goodbye!", Color.GREEN, bold=True)
            break

def main():
    parser = argparse.ArgumentParser(description="Personal Linux Help Desk & Command Catalog")
    parser.add_argument("command", nargs="?", help="Direct lookup of a command (e.g. 'help-desk tar')")
    parser.add_argument("--import", dest="import_file", help="Import AI-generated JSON file into database")
    parser.add_argument("--export-prompt", nargs="*", help="Export prompt for AI with specified packages")
    parser.add_argument("--diff", action="store_true", help="List packages not yet documented in Help Desk")
    parser.add_argument("--list", action="store_true", help="List all documented commands")

    args = parser.parse_args()

    init_db()
    auto_seed_if_empty()

    if args.import_file:
        import_json_data(args.import_file)
        sys.exit(0)

    if args.diff:
        undoc = get_undocumented_packages()
        if undoc:
            print(f"Undocumented packages ({len(undoc)}):")
            for u in undoc:
                print(f"  • {u}")
        else:
            print("All known packages are already documented.")
        sys.exit(0)

    if args.export_prompt is not None:
        pkgs = args.export_prompt if len(args.export_prompt) > 0 else get_undocumented_packages()
        if not pkgs:
            pkgs = scan_system_packages()
        prompt = generate_ai_prompt(pkgs)
        print(prompt)
        sys.exit(0)

    if args.list:
        rows = get_all_commands()
        print(f"Documented commands ({len(rows)}):")
        for r in rows:
            print(f"  {r['command']:<12} [{r['category']}] : {r['summary_ja']}")
        sys.exit(0)

    if args.command:
        row = get_command(args.command)
        if row:
            display_command_card(row)
        else:
            cprint(f"Command '{args.command}' not found in Help Desk.", Color.RED)
            res = search_commands(args.command)
            if res:
                cprint("Did you mean one of these?", Color.YELLOW)
                for r in res:
                    cprint(f"  • {r['command']} ({r['summary_ja']})", Color.WHITE)
        sys.exit(0)

    main_interactive()

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        cprint("\n\nExited help-desk.", Color.YELLOW)
        sys.exit(0)
