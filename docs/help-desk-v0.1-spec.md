# Personal CLI Help Desk v0.1 规格

状态：Ready for Delivery  
产品版本：0.1  
Cargo 版本：0.1.0  
Git tag：`v0.1.0`

## 1. 产品定位

Personal CLI Help Desk 是一个单语言、纯 Rust、本地优先的个人命令速查工具。它解决的不是“完整替代 man 手册”，而是将用户实际安装、定义和经常使用的命令聚合为一份可搜索、可补充说明、可重复导入的个人命令目录。

核心价值循环：

```text
扫描全局 CLI 与 Shell 定义
→ 保存带包版本的只读基线快照
→ 用户审查并导出脱敏的 AI 输入
→ AI 补充简短说明、语法与常用示例
→ 校验并导入本地目录
→ 通过 hdesk <command> 随时查询
→ 用户在三至六个月后按需手动复扫
→ 比较包版本、命令集合与 Shell 定义指纹
→ 只复核并更新受影响的命令
```

## 2. v0.1 范围

### 2.1 包含

- Rust CLI 可执行文件 `hdesk`，兼容命令名 `help-desk` 可作为软链接。
- 单语言、任意 UTF-8 内容，不维护并行的日语/英语字段。
- 直接查询、列表、搜索、扫描、JSON 导出、JSON 校验和导入。
- 保存不可变的环境快照，并按用户要求比较两个快照中的包版本、命令集合和 Shell 定义变化。
- 聚合以下命令来源：
  - PATH 中的用户级或全局独立二进制；
  - Zsh alias；
  - 用户编写的 Zsh function；
  - Python 全局或用户级安装中实际暴露的 CLI；
  - pipx 与 uv 持久安装的全局工具；
  - `cargo install` 安装的 Rust 二进制；
  - npm 全局包实际暴露的 `bin`；
  - Composer global 包实际暴露的 CLI；
  - Go 全局二进制作为可选扫描源。
- 生成适合外部 AI 补全说明的脱敏 JSON。
- Linux AMD64 与 macOS Apple Silicon 二进制发布。
- SHA-256、GPG detached signature 与 GitHub Artifact Attestation。

### 2.2 明确排除

- 不扫描任何项目内的 `.venv`、`venv`、`node_modules`、`vendor`、`target` 或 Go module 依赖。
- 不遍历用户项目以推断依赖。
- 不调用付费或在线 AI API；AI 补全由用户在外部完成。
- 不做多语言切换。
- 不做 TUI、鼠标操作或双栏界面。
- 不做 Linux 系统更新、包升级或自动安装。
- 不做 daemon、cron、登录触发器或每日轮询；扫描、比较和 AI 更新均由用户手动发起。
- 不做旧 Python 数据库恢复、旧数据迁移或 Python fallback。
- 不删除、更新或卸载任何已安装工具。
- 不自动执行任意发现到的命令。
- v0.1 不承诺 Windows 二进制。

当前尚无正式数据，因此 v0.1 使用新的数据目录和数据库，不读取或修改旧的 `~/.help_desk.db`。

## 3. 能力地图

| 功能 ID | L1 能力 | L2 模块 | L3 功能 | 产品状态 |
|---|---|---|---|---|
| `CAT-QUERY-001` | 命令目录 | 查询 | 按命令名显示速查卡 | Ready for Delivery |
| `CAT-QUERY-002` | 命令目录 | 发现 | 列表、标签与关键词搜索 | Ready for Delivery |
| `CAT-EDIT-001` | 命令目录 | 维护 | 新增、编辑和确认删除 | Ready for Delivery |
| `DISC-SCAN-001` | 环境发现 | 二进制 | 发现用户级及全局 CLI 二进制 | Ready for Delivery |
| `DISC-SHELL-001` | 环境发现 | Zsh | 发现 alias 与用户 function | Ready for Delivery |
| `DISC-ECO-001` | 环境发现 | 生态工具 | 发现 Python、Rust、Node、PHP 全局 CLI | Ready for Delivery |
| `DISC-GO-001` | 环境发现 | Go | 从全局 bin 识别 Go CLI | Deferred |
| `DISC-SNAP-001` | 环境发现 | 快照 | 保存包版本、命令集合与采集完整性 | Ready for Delivery |
| `DISC-DIFF-001` | 环境发现 | 比较 | 手动比较两个环境快照 | Ready for Delivery |
| `AGG-NORM-001` | 聚合 | 规范化 | 去重、冲突保留与来源归类 | Ready for Delivery |
| `AI-EXCH-001` | AI 协作 | 导出 | 生成脱敏的 AI 输入 JSON | Ready for Delivery |
| `AI-EXCH-002` | AI 协作 | 导入 | 校验并事务导入 AI 丰富结果 | Ready for Delivery |
| `AI-EXCH-003` | AI 协作 | 增量更新 | 只导出需复核的变化项并定向导入 | Ready for Delivery |
| `CAT-REFRESH-001` | 命令目录 | 版本维护 | 保留旧内容并更新指定命令卡 | Ready for Delivery |
| `REL-BIN-001` | 分发 | 二进制 | Linux AMD64 与 macOS ARM64 Release | Ready for Delivery |
| `REL-TRUST-001` | 分发 | 完整性 | SHA-256、GPG 和构建来源验证 | Ready for Delivery |

## 4. 用户可见 CLI

### 4.1 日常查询

```bash
hdesk tar
hdesk show tar
hdesk list
hdesk list --kind alias
hdesk list --source cargo
hdesk search compression
hdesk search 压缩
```

### 4.2 手工维护

```bash
hdesk add tar
hdesk edit tar
hdesk remove tar
```

删除操作必须显示目标并要求明确确认；非交互场景必须显式传入 `--yes`。任何删除仅影响 hdesk 自己的目录记录，不得删除系统命令、包、alias 或 function。

### 4.3 环境扫描与 AI 协作

```bash
hdesk scan --output inventory.json
hdesk scan --snapshot --output inventory.json
hdesk snapshot list
hdesk diff
hdesk diff --from <snapshot-id> --to <snapshot-id>
hdesk shell-init zsh
hdesk inventory inventory.json
hdesk export-ai inventory.json --output ai-input.json
hdesk export-ai --changes-only --from <snapshot-id> --output ai-update.json
hdesk validate enriched-catalog.json
hdesk import enriched-catalog.json
```

`scan --snapshot` 保存一次不可变的本机事实快照；只有完整快照才可成为新的默认比较基线，不完整快照仅保留作诊断。`diff` 默认比较最近两次完整快照；显式 `--from/--to` 时按指定快照比较。快照不会定时生成，也不会自动清理；可另行提供显式的 `snapshot prune --keep <n>`，执行前必须展示目标并确认。

### 4.4 诊断

```bash
hdesk --help
hdesk --version
hdesk doctor
```

`--help` 与 `--version` 必须无数据库写入副作用。

## 5. 发现源与提取策略

### 5.1 独立二进制与 PATH 命令

扫描范围默认仅包括：

- `~/.local/bin`
- `/usr/local/bin`
- Homebrew prefix 的 `bin`
- Cargo、pipx、uv、Composer、npm 和 Go 已知的全局 bin 目录
- 用户配置中显式追加到 PATH 的其他目录

不默认遍历 `/usr/bin` 与 `/bin` 的全部系统命令，以避免生成不可审查的大清单。基础系统命令可通过内置种子或用户显式传入 `--include-system` 添加。

每个候选必须满足：

- 是普通文件或可解析的符号链接；
- 对当前用户可执行；
- 记录解析后的真实路径；
- 不执行该文件；
- 不跟随到项目级依赖目录。

### 5.2 Zsh alias

独立进程无法直接读取父 Zsh 会话中的 alias。推荐通过 `hdesk shell-init zsh` 输出一段可审查的 Shell 集成代码，由用户在当前 Zsh 会话中显式加载；集成函数只读取运行时 alias 元数据并通过 stdin 交给 Rust 聚合器，而不执行 alias。记录：

- alias 名称；
- 展开内容；
- 是否为 global alias；
- 可确定时记录来源配置文件。

不得为了方便而自动运行 `zsh -lic` 并加载真实启动文件，因为 `.zshrc` 本身可以包含任意有副作用的命令。作为备用模式，只能在用户显式传入 `--allow-source <file>` 后，在隔离子 Shell 中 source 指定文件，并提前显示风险提示。

alias 展开内容可能包含 token、密码、私有 URL 或用户名，因此原始 inventory 只保存在本地；AI 导出前必须执行脱敏。

### 5.3 Zsh function

通过当前 Zsh 会话的运行时函数表获取 function 名称，并优先使用 `functions_source` 识别定义来源。与 alias 一样，数据由显式加载的 Shell 集成传给 Rust，不自动启动并执行真实 `.zshrc`。只接受来源属于用户配置范围的函数，例如：

- `~/.zshrc`
- `~/.zshenv`
- `~/.zprofile`
- `~/.config/zsh/**`
- 用户显式配置的 dotfiles 目录

默认排除 Oh My Zsh、插件、补全系统和系统级函数；用户可以通过配置增加允许的来源目录。

function body 不直接进入最终目录。AI 导出最多包含经过脱敏的定义摘要或用户明确批准的函数体。

### 5.4 Python 全局 CLI

Python 扫描的判断对象是“实际暴露的命令”，不是所有已安装库。

来源包括：

1. `pipx list --output json` 返回的持久安装工具和 apps；
2. uv 持久安装的 tools 与 executable directory；
3. 不处于 virtualenv 的 Python 解释器，通过 `importlib.metadata` 查询 `console_scripts`；
4. 用户 site-packages 中的 `console_scripts`。

必须排除：

- 当前项目 `.venv`；
- Poetry/Pipenv/Conda 的项目环境；
- `uvx` 或其他一次性缓存工具，除非已经持久安装；
- 没有 console/script entry point 的普通库。

### 5.5 Rust 全局 CLI

使用 `cargo install --list` 获取 Cargo 全局安装的 crate、版本和二进制名称，并结合 Cargo install root 的 `bin` 目录验证二进制实际存在。

不得扫描项目 `target` 目录，也不得把普通 Rust library crate 误认为 CLI。

### 5.6 Node 全局 CLI

v0.1 首先支持 npm：

- 获取 npm global prefix/root；
- 列出顶层全局包；
- 读取每个全局包 `package.json` 的 `bin` 字段；
- 只收录实际存在的全局 bin 链接。

不得读取项目内 `node_modules/.bin`。pnpm、Yarn 与 Bun 可采用同一“全局包 + 实际 bin”适配器模型后续补充，不得通过扫描整个 Home 猜测安装来源。

### 5.7 PHP Composer global CLI

通过以下信息发现：

- Composer global home；
- `composer global config bin-dir --absolute`；
- global 项目的已安装包；
- global bin-dir 中实际存在的代理脚本或链接。

只收录 Composer global 环境，不扫描任何项目的 `vendor/bin`。

### 5.8 Go 全局 CLI

Go 在 v0.1 中为 Deferred。若提前实现，只允许扫描 `$GOBIN` 或 `$(go env GOPATH)/bin`，并使用 `go version -m <binary>` 尝试读取模块路径和版本；无法确定来源时标记为低置信度，不得猜测包名。

## 6. 聚合与冲突规则

“命令名相同”不代表同一条记录。聚合器必须保留所有来源候选，并标记当前 Zsh 环境实际解析到的有效命令。

典型冲突：

```text
ls 可能同时存在：
- Zsh alias
- Zsh function
- /bin/ls
```

规则：

1. 保留每个候选的唯一来源身份。
2. 额外生成一个 effective resolution，表示当前 Shell 会执行哪一个。
3. 优先级依据实际 Shell 解析结果，不在代码中硬编码猜测。
4. 同一路径被多个生态重复发现时合并来源证据，不重复生成命令卡。
5. 包名和命令名分离。例如 `ripgrep` 提供 `rg`，Python 包可能提供多个 console scripts。
6. 发现结果不自动成为可信目录；必须经过 review/import 状态转换。

发现置信度：

- `high`：包管理器明确声明并且 bin 实际存在；
- `medium`：PATH 中存在可执行文件，但包来源无法完全确认；
- `low`：只能从路径或二进制元数据推断。

## 7. 版本快照、差异与 JSON 交换

### 7.1 版本与来源记录

除 Zsh alias/function 外，每个发现到的 CLI 都应尽可能关联其全局安装包及版本：

- Python：distribution 名称与版本、解释器、安装范围、`console_scripts`；pipx/uv 另记录 tool 版本与 exposed apps；
- Cargo：crate 名称、版本、安装来源、install root 与 binary 集合；
- npm：全局 package 名称、版本与 `bin` 映射；
- Composer：global package 名称、版本与 global bin 映射；
- PATH/独立二进制：能可靠解析时记录 apt、dnf/rpm 或 Homebrew 所属包及版本；不能解析时保留路径、文件指纹和 `version: null`，不得猜测；
- Zsh alias/function：不伪造包版本，记录脱敏后的定义指纹、来源文件和可获得的修改时间。定义正文默认只保留在本地原始快照中。

包事实是一等数据，不仅是 command 的重复属性。只记录实际提供全局 CLI 的包，至少包含 `ecosystem`、`package`、原始 `version` 字符串、`scope`、安装来源、命令集合、install path、发现方法、置信度与采集时间。版本排序优先使用生态自身可靠规则；无法可靠排序时只标记 `version_changed`，不得强行套用通用 SemVer 判断升级或降级。

### 7.2 快照与差异语义

首次扫描建立基线；以后由用户按需手动建立新快照。产品不假定固定周期，典型使用间隔可以是三至六个月。每个快照包含 host、采集时间、各适配器状态和完整性标记。

差异状态包括：

- `unchanged`、`added`、`removed`；
- `upgraded`、`downgraded`、`version_changed`；
- `provider_changed`、`commands_changed`；
- alias/function 的 `definition_changed`。

约束：

1. 包版本变化不等于 CLI 用法变化，只能把相关命令标为 `needs_review`。
2. 只有命令集合差异、用户批准采集的 help excerpt、发布说明或其他明确证据，才能支持修改命令卡；AI 不得猜测功能变化。
3. 包或命令在新快照中消失时，不自动删除目录记录，只标记 `unavailable`/`stale_pending_confirmation`，由用户确认后归档或删除。
4. 任一适配器失败或快照不完整时，不得把该来源中未出现的包判定为 `removed`。
5. 重复扫描且事实未变化时，diff 必须为空，也不得生成 AI 更新批次。
6. 快照默认一直保留；清理只能由用户显式执行，不能后台自动删除。

### 7.3 原始发现文件 `inventory-v0.1.json`

```json
{
  "schema_version": "0.1",
  "generated_at": "2026-11-10T00:00:00Z",
  "snapshot_id": "20261110T000000Z-linux-x86_64",
  "complete": true,
  "host": {
    "os": "linux",
    "arch": "x86_64",
    "shell": "zsh"
  },
  "packages": [
    {
      "id": "cargo:ripgrep",
      "ecosystem": "cargo",
      "name": "ripgrep",
      "version": "14.1.1",
      "scope": "global",
      "commands": ["cargo:ripgrep:rg"],
      "discovery_method": "cargo install --list",
      "confidence": "high"
    }
  ],
  "commands": [
    {
      "id": "cargo:ripgrep:rg",
      "name": "rg",
      "kind": "rust_binary",
      "provider": {
        "ecosystem": "cargo",
        "package": "ripgrep",
        "version": "14.1.1",
        "scope": "global"
      },
      "executable_path": "/home/user/.cargo/bin/rg",
      "definition": null,
      "source_files": [],
      "effective": true,
      "confidence": "high",
      "discovery_method": "cargo install --list",
      "warnings": []
    }
  ]
}
```

原始 inventory 是本机事实快照，不允许 AI 修改后再当作发现事实导入。

### 7.4 AI 输入 `ai-input-v0.1.json`

AI 输入仅包含生成说明所需的信息：

- 命令名和类型；
- 包名、版本与生态；
- 脱敏后的 alias/function 摘要；
- 可选的、用户批准采集的 help excerpt；
- 来源置信度。

`--changes-only` 更新文件还必须包含旧/新包版本、旧/新命令集合、变化原因、现有目录条目和用户批准的证据。未变化的命令不得进入该批次。

必须移除或遮蔽：

- access token、API key、密码；
- 私有仓库凭据；
- 带凭据的 URL；
- SSH 私钥内容；
- 用户未批准的完整函数体；
- 与命令说明无关的绝对私人路径。

### 7.5 AI 输出/导入文件 `catalog-v0.1.json`

```json
{
  "schema_version": "0.1",
  "entries": [
    {
      "command_id": "cargo:ripgrep:rg",
      "name": "rg",
      "kind": "rust_binary",
      "summary": "递归搜索文件内容的快速命令行工具。",
      "syntax": "rg [OPTIONS] PATTERN [PATH ...]",
      "examples": [
        {
          "description": "在当前目录搜索 TODO",
          "command": "rg TODO"
        }
      ],
      "notes": [],
      "tags": ["search", "files"],
      "provider": {
        "ecosystem": "cargo",
        "package": "ripgrep",
        "version": "14.1.1"
      }
    }
  ]
}
```

导入前必须：

- 校验 schema version；
- 拒绝未知的顶层结构；
- 校验 command_id 必须来自本次或已保存的 inventory；
- 限制字段长度和示例数量；
- 拒绝空命令名；
- 输出逐条错误；
- 所有条目通过后才在单个事务中导入。

增量导入还必须限制为更新文件中明确列出的 command_id；未列出的目录内容保持原样。版本变化但证据不足的条目维持原说明，只更新本地 `needs_review` 状态。

目录条目必须保留其说明最后一次确认时对应的 provider 版本（`documented_against_version`）。这样后续快照即使已发现新版本，也能明确区分“当前安装版本”与“当前说明所依据的版本”。

## 8. 安全规则

### 8.1 扫描安全

- 扫描默认只读。
- 不得执行发现到的 alias、function 或 binary。
- 不得默认对所有命令运行 `--help`，因为并非所有程序的 `--help` 都保证无副作用。
- 如果提供 help 采集，必须由用户选择命令、设置超时、禁止 stdin，并显示即将执行的命令。
- 不得访问网络来补充包信息，除非用户显式授权。
- 不得修改 PATH、Shell 配置或包管理器状态。

### 8.2 数据安全

- AI 导出前执行敏感信息检测与脱敏。
- 检测到疑似秘密时默认排除该字段，并列入 warnings。
- 原始 inventory 与 AI 输入分开保存。
- 导入必须事务化；失败时目录保持原状。
- 本工具自身的 remove 只能删除目录记录，不能删除系统资源。

## 9. 数据位置

v0.1 使用新位置：

- Linux：`${XDG_DATA_HOME:-~/.local/share}/hdesk/catalog.db`
- macOS：`~/Library/Application Support/hdesk/catalog.db`

配置位置：

- Linux：`${XDG_CONFIG_HOME:-~/.config}/hdesk/config.toml`
- macOS：`~/Library/Application Support/hdesk/config.toml`

不得自动读取、迁移或删除旧的 `~/.help_desk.db`。

## 10. 跨平台发布

首发产物：

| 平台 | Rust target | 文件 |
|---|---|---|
| Linux AMD64 | `x86_64-unknown-linux-musl` | `hdesk-v0.1.0-x86_64-unknown-linux-musl.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `hdesk-v0.1.0-aarch64-apple-darwin.tar.gz` |

GitHub Release 还必须包含：

```text
SHA256SUMS
SHA256SUMS.asc
release-key.asc
```

发布要求：

1. Git tag 为 `v0.1.0`，必须与 Cargo package version 一致。
2. 所有构建使用 `cargo build --locked --release`。
3. Linux 产物在 x86_64 runner 构建并确认不存在非预期 glibc 依赖。
4. macOS 产物在 ARM64 runner 构建并确认是 Mach-O arm64。
5. 每个压缩包进入 `SHA256SUMS`。
6. 使用专用 GPG release signing subkey 对 `SHA256SUMS` 创建 detached signature。
7. 发布 GitHub Artifact Attestation。
8. GitHub Actions 使用最小权限并将第三方 actions 固定到完整 commit SHA。
9. Release 先创建为 draft，签名和验收完成后再公开。

推荐签名过程：CI 构建并产生 attestation，人工下载并验证后使用离线或硬件保护的 GPG signing subkey 签名，再上传签名并发布。GPG 能验证发布文件是否被修改；Artifact Attestation 用于补充证明构建来源。

macOS Developer ID 签名与 notarization 不属于 GPG 的替代品。v0.1 可先完成 GPG 与 attestation；若面向更多 Mac 用户公开分发，再加入 Apple code signing/notarization 门禁。

## 11. 分阶段交付

### M1：精简 Rust CLI

- 移除正式运行链中的 Python fallback。
- CLI 参数、退出码和无副作用 `--help/--version`。
- 单语言目录、查询、列表、搜索和手工维护。
- 新数据目录，不处理旧数据。

结束条件：可在临时数据目录完成 add → show → search → remove 全流程。

### M2：全局 CLI 发现与聚合

- PATH/user bin、Zsh、Python、Cargo、npm、Composer 适配器。
- 全局范围过滤、来源证据、冲突保留、置信度。
- 原始 inventory JSON、包版本事实、不可变快照与完整性状态。

结束条件：在合成测试环境和真实只读环境中，项目级依赖不会进入清单；相同环境的重复快照产生空 diff，适配器失败不会产生伪 `removed`。

### M3：AI 交换与安全导入

- 脱敏 AI 输入。
- JSON Schema、validate、事务导入。
- AI 丰富结果可被 CLI 查询。
- 基于两个快照生成 changes-only 批次，只允许定向更新受影响命令。

结束条件：正常批次完整导入；包含秘密、未知 command_id 或半损坏对象的批次被拒绝且数据库不变化。

### M4：跨平台 Release

- Linux AMD64 musl、macOS ARM64。
- checksums、GPG、Artifact Attestation、draft release。
- 安装与验证说明。

结束条件：两平台从 GitHub Release 下载后均可独立验证并完成 smoke test。

## 12. 验收标准

### 功能

- `hdesk <command>` 在 100ms 级别内返回本地速查卡，不访问网络。
- 扫描只报告全局或用户级 CLI，不包含测试项目中的 `.venv/node_modules/vendor/target`。
- alias 与同名 binary 同时存在时不丢失任一来源，并能指出当前有效定义。
- 一个包提供多个命令时能生成多个 command candidate。
- AI 输出无法伪造不存在于 inventory 的 command_id。
- 重复扫描和重复导入具有幂等结果。
- 所有可归属的全局 CLI 均记录包版本；不可可靠归属时明确记录未知，不猜测版本。
- 包版本变化但命令集合不变时只进入 `needs_review`，不会自动改写说明。
- changes-only 导出不包含未变化的命令，增量导入不修改批次外条目。

### 失败与边界

- 未安装某个生态工具时对应适配器显示 skipped，而不是整体失败。
- Zsh 配置加载失败时报告错误和来源，不执行部分导入。
- 数据库或 JSON 不可读时返回非零退出码和可操作错误。
- 所有扫描源失败时仍能输出结构合法的空 inventory。
- 部分适配器失败时快照标记为不完整，diff 不得将该来源的缺失项判定为删除。
- 包或命令消失时保留旧目录条目并等待用户确认，不自动删除或归档。
- 任何失败都不得改变包、Shell 配置或旧数据库。

### 工程门禁

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

还必须包含：

- 聚合去重和冲突测试；
- 项目级依赖排除测试；
- Zsh alias/function fixture；
- Python console_scripts fixture；
- Cargo/npm/Composer 输出 fixture；
- 包升级、降级、未知版本排序与命令集合变化 fixture；
- 完整/不完整快照、空 diff、定向 AI 更新和目录保留测试；
- 脱敏测试；
- JSON Schema 与事务回滚测试；
- Linux 和 macOS CLI smoke tests；
- Release checksum、GPG 与 attestation 验证。

## 13. 当前代码基线

截至 2026-11-10：

- 现有 Rust crate 可在 M1 Mac 构建为 Mach-O arm64。
- `cargo test --locked` 成功，但当前为 0 个测试。
- 严格 `cargo clippy -- -D warnings` 尚未通过。
- 当前代码仍包含多语言、TUI、AI Prompt 与 Python fallback。
- 当前没有 GitHub Actions Release、GPG 签名或 attestation 流程。
- Linux AMD64 musl 尚未完成实际构建和运行验证。

这些现状只能说明迁移可行，不能视为 v0.1 已达到发布条件。

## 14. 设计依据

- [Cargo install 与 `cargo install --list`](https://doc.rust-lang.org/stable/cargo/commands/cargo-install.html)
- [Python `importlib.metadata` 与 `console_scripts`](https://docs.python.org/3/library/importlib.metadata.html)
- [pipx JSON 输出](https://pipx.pypa.io/stable/reference/json-output.html)
- [uv 持久工具与可执行文件](https://docs.astral.sh/uv/concepts/tools/)
- [npm package.json `bin`](https://docs.npmjs.com/cli/v8/configuring-npm/package-json/)
- [Composer global CLI](https://getcomposer.org/doc/03-cli.md#global)
- [Composer vendor binaries](https://getcomposer.org/doc/articles/vendor-binaries.md)
- [Go `go version -m`](https://go.dev/ref/mod#go-version-m)
- [Zsh 官方手册](https://zsh.sourceforge.io/Doc/Release/)
- [Rust 平台支持](https://doc.rust-lang.org/rustc/platform-support.html)
- [GitHub Actions 安全使用](https://docs.github.com/en/actions/reference/security/secure-use)
- [GitHub Artifact Attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
- [Apple 软件签名与 notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
