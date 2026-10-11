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

### 7.4. Personal CLI Help Desk & Incremental Update Trigger (help_desk.py / sys_update.sh)
- **Problem**: Standard `man` pages and `--help` outputs are dense, text-heavy, mostly in English, and lack practical examples. Developers need a localized (Japanese) cheat-sheet catalog tailored exclusively to the tools they have installed, including custom scripts.
- **Architecture**:
  1. **Zero External Dependencies**: Pure Python 3 and SQLite (`~/.help_desk.db`) storing structured Japanese reference cards.
  2. **Multi-dimensional Exploration**:
     - Browse by Category (`ファイル操作・圧縮`, `ネットワーク・通信`, `自作開発ツール`, `システム監視・プロセス`, `開発・エディタ`, `モダンCLI拡張`).
     - Browse Alphabetically (A-Z).
     - Full-text keyword search across commands, Japanese summaries, descriptions, and option flags.
     - Direct cheat card lookup: `help-desk <command>` (e.g. `help-desk tar` or `hdesk req`).
  3. **Strict JSON Schema (`help_schema.json`)**: Formally defines the structure of command documentation (syntax, flags, practical examples).
  4. **Incremental Web AI Prompt Generator**:
     - Compares system/repo packages against Help Desk SQLite records.
     - Extracts only incremental (undocumented or upgraded) packages to prevent redundant prompt tokens.
     - Generates copy-pasteable prompt and schema for web AI (ChatGPT/Claude/Gemini).
  5. **Batch Importer**: `help-desk --import <file.json>` imports AI-generated batches and refreshes the catalog instantly.
  6. **Distro-Aware System Update Trigger (`sys_update.sh` / `sys-update`)**:
     - Automatically identifies package manager (`dnf`, `apt-get`, `pacman`, `zypper`, `apk`, `brew`).
     - Performs system upgrades and calculates which packages were upgraded.
     - Interactively prompts to update the Help Desk and generates the AI prompt for incremental packages.
  7. **Global CLI Aliases**: Integrated into `dotfiles/.zshrc` and `restore.sh` as `help-desk`, `hdesk`, and `sys-update`.
