# Geek Workstation: Cross-Environment & Automation Blueprint (WSL & macOS)

This document specifies the requirements, workflows, and custom tooling for building an automated, cross-distribution Linux/WSL development environment with high-efficiency terminal productivity.

---

## 1. Operating System & User Management (OS & System Core)
**Goal**: Ensure clean privilege separation and personalized presets on any new WSL instance.
- [ ] **Base OS**: AlmaLinux 9.7 (RHEL-based, using `dnf`).
- [ ] **User & Privilege Automation**:
  - Configurable username (defaults to `ouki`).
  - Auto-grant administrative privileges (`sudo` / `wheel` group).
  - Interactive password prompt (reset or retain current).
- [ ] **WSL Boot Configuration**:
  - Automatically configure `/etc/wsl.conf` to launch with the configured default user (non-root).

---

## 2. Terminal User Experience (Terminal UX)
**Goal**: Beautiful, high-density, and ergonomic command-line workflow.
- [ ] **Primary Shell**: `zsh` (auto set as default shell).
- [ ] **Modern CLI Replacements**:
  - `eza`: Modern `ls` alternative with Nerd Fonts icon support.
  - `bat`: Modern `cat` alternative with syntax highlighting.
- [ ] **System Status & Process Monitoring**:
  - `htop` / `btop`: Aesthetic terminal monitors.
  - **Disk Analysis**: `ncdu` / `dust` visual disk usage analyzers.
  - **Process Management**: Enhanced process viewers.

---

## 3. Programming Languages & Toolchains (Dev Toolchains)
**Goal**: Rapid readiness of primary programming environments and global dependencies.
- [ ] **Language SDKs**:
  - Node.js, PHP, C# (.NET SDK), Python, Go, Rust, Ruby, Dart.
- [ ] **Global Package Manager Synchronization**:
  - **Node.js**: Global `npm` / `pnpm` / `yarn` packages.
  - **PHP**: Global `Composer` packages.
- [ ] **Modern Version Control**:
  - `Git` and global `~/.gitconfig` restoration.
  - `jj` (Jujutsu): Next-generation Git-compatible VCS.

---

## 4. Text Editing, Media Viewing & Search (Editors & Search)
**Goal**: Top-tier editing and navigation directly within the terminal.
- [ ] **Editor**: `Neovim` (NVIM).
- [ ] **In-Terminal Viewers**:
  - **Markdown Viewer**: Formatted Markdown rendering in terminal (`glow`).
  - **Image Viewer**: Lightweight image preview compatible with WSL.
- [ ] **Fast Search Workflows**:
  - `fzf`: Interactive fuzzy finder for files, history, and repos.
  - `ripgrep` (`rg`): High-performance regular expression search.
- [ ] **Archiving Suite**: `unzip`, `tar`, `7z`.

---

## 5. Containerization & Data Persistence (Docker & Data)
**Goal**: Safe migration and restoration of local container assets.
- [ ] **Container Engine**: Native Docker CE.
- [ ] **Private Registry & Database Persistence**:
  - **Local Registry**: Restoration of local Docker image registry.
  - **Personal DB Restoration**: Full volume/image migration of personal data databases.

---

## 6. Secrets, Credentials & Workspace Backup (Secrets & Workspaces)
**Goal**: Prevent loss of digital credentials and untracked work-in-progress files.
- [ ] **Credentials Injection**:
  - Backup & restore `~/.ssh/` key pairs.
  - Restore database connection profiles (`.pgpass`, cloud server configs).
- [ ] **Non-Git Workspace Archiving**:
  - Scan `~/workspace` directory.
  - Package and backup unversioned directories, drafts, and loose project files.

---

## 7. Advanced Custom Workflows & Tools (Advanced Workflows)
Specialized tools built into the repository to enhance daily developer productivity.

### 7.1. Cross-Platform Notification Dispatcher (WSL ➡️ Windows)
- **Problem**: Native Windows toast notifications block chat inputs; standard webpage WebSockets disconnect upon page refresh.
- **Architecture**:
  1. WSL file watcher daemon (`inotify`).
  2. Local WebSocket message broadcast hub.
  3. **Manifest V3 Browser Extension (Client)**: Persistent background `Service Worker` connection displaying non-intrusive banner notifications injected directly into active tabs.

### 7.2. Safe Pull-Backup Deployer for Legacy Servers (deploy.py / rollback.py)
- **Problem**: Deploying hundreds of files over raw FTP/SFTP without server-side scripts risks silent overwrites.
- **Architecture**:
  1. **Pre-flight Verification**: Prints target host, protocol, and real remote `pwd` with mandatory confirmation.
  2. **Pull-before-push Backup**: Automatically pulls old files to `~/.deploy_backups/<repo>/<timestamp>/` and verifies downloaded sizes.
  3. **Atomic Deployment**: Uploads to `.deploy_tmp`, checks size integrity, and executes atomic renames.
  4. **Manifest & 1-Click Rollback**: Generates `manifest.json` enabling `rollback.py` to restore previous files or delete newly added files.

### 7.3. Interactive CLI HTTP Client & Hub (http_cli.py / req)
- **Problem**: `curl` / `HTTPie` require remembering complex command flags; GUI clients like Postman are heavyweight and outside terminal workflows.
- **Architecture**:
  1. **Zero External Dependencies**: Built with pure Python 3 standard libraries (`urllib`, `sqlite3`, `ssl`, `json`).
  2. **Single-Keypress Confirmation & Step Navigation**: Instant single-key responses (`y`/`n`/`b` to go back) without hitting Enter; steppable wizard with dedicated Query Parameters editor.
  3. **Project Variables ({{var}})**: Shared project properties (`baseUrl`, `apiKey`, `token`) auto-interpolated across URL, Headers, and Body.
  4. **SQLite Lifecycle Persistence**: Automatic logging of latency, status, headers, and responses to `~/.http_cli.db`.
  5. **Domain History Search & 1-Click Replay**: Filter past requests by domain keyword and replay immediately.
  6. **Native File Uploads**: Generates standard `multipart/form-data` and binary streams from local Linux/WSL file paths.
  7. **Terminal HTML Webpage Preview & Engine Switcher**:
     - Automatically detects installed terminal HTML renderers (`w3m`, `lynx`, `elinks`, `browsh`, and pure-Python Built-in engine).
     - Persistently stores preferred rendering engine in SQLite `settings` table and auto-renders future HTML responses accordingly.
     - Provides an interactive post-response action menu to switch engines, immediately re-render, inspect raw HTML, or cleanly exit.
  8. **Global CLI Alias**: Integrated into `dotfiles/.zshrc` and `restore.sh` as `req` / `http-cli`.

### 7.4. Personal CLI Help Desk v0.1 (Rust)
- **Problem**: Standard help pages do not reflect the user's actual command habits, custom Zsh functions, aliases, or globally installed language tools. The user needs one local catalog of the CLI commands that are really available and personally useful.
- **Version boundary**:
  - Product milestone: `0.1`; Cargo version: `0.1.0`; Git tag: `v0.1.0`.
  - Rust-only runtime. Python is not a supported entry point or fallback.
  - Single-language UTF-8 catalog. Multi-language UI, TUI, package upgrades, online AI APIs, and legacy database recovery are outside v0.1.
  - No daemon, cron, login-triggered scan, or daily polling. Inventory refresh and comparison are manually initiated, typically when the user chooses to review changes after several months.
  - There is no production catalog data yet. The new implementation must not read, migrate, modify, or delete the legacy `~/.help_desk.db`.
- **Core workflow**:
  1. Scan global/user-level CLI sources without modifying the environment.
  2. Produce an immutable, reviewable snapshot with command origin, provider package version, command set, path, adapter status, and confidence.
  3. Export a minimized and redacted JSON file for an external AI to add a short summary, syntax, tags, and practical examples.
  4. Validate the enriched JSON and import the complete batch transactionally.
  5. Query the local catalog with `hdesk <command>`, `hdesk list`, and `hdesk search`.
  6. On a later manual scan, compare snapshots and export only added or changed commands for review; unchanged catalog entries remain untouched.
- **Discovery sources**:
  - User/global executable directories and standalone binaries.
  - Zsh aliases and user-authored functions, excluding framework/plugin/system functions by default.
  - Python global/user `console_scripts`, pipx installed apps, and persistently installed uv tools.
  - Rust binaries installed by `cargo install`.
  - npm global packages that actually expose a `bin` entry.
  - Composer global packages that actually expose a CLI in the global bin directory.
  - Go global binary discovery is deferred unless explicitly brought into v0.1.
- **Scope guard**: Never scan project `.venv`, `node_modules`, Composer `vendor`, Cargo `target`, or per-project dependency graphs. Package names and command names are separate concepts, and one package may provide several commands.
- **Safety**:
  - Scanning is read-only and does not install, update, remove, or execute discovered tools.
  - Raw alias/function definitions remain local; AI export must redact tokens, credentials, private URLs, SSH keys, and unapproved function bodies.
  - Import is all-or-nothing after schema validation. AI output cannot invent command IDs that were not present in the inventory.
  - A package version change marks its commands as `needs_review`; it does not prove that CLI behavior changed. Descriptions are updated only with explicit evidence.
  - A missing package or command is never auto-deleted from the catalog. Incomplete adapter results cannot be interpreted as removals.
- **Release targets**:
  - Linux AMD64: `x86_64-unknown-linux-musl`.
  - macOS Apple Silicon: `aarch64-apple-darwin`.
  - GitHub Release assets include both binaries, `SHA256SUMS`, `SHA256SUMS.asc`, the public release key, and GitHub Artifact Attestations.
  - The milestone is complete only after native-platform smoke tests, GPG verification, checksum verification, and attestation verification pass.
- **Detailed specifications**:
  - Product and delivery specification: [`docs/help-desk-v0.1-spec.md`](docs/help-desk-v0.1-spec.md)
  - AI implementation prompt: [`docs/prompts/help-desk-v0.1-implementation.md`](docs/prompts/help-desk-v0.1-implementation.md)
