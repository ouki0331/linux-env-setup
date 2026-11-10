# Personal CLI Help Desk v0.1 — AI 开发提示词

将下面整段提示词交给负责开发的 AI。开发前必须同时阅读 `docs/help-desk-v0.1-spec.md`，以该规格为产品行为的唯一来源。

---

你正在维护仓库 `linux-env-setup`。请将现有 Personal CLI Help Desk 收敛并实现为 v0.1.0 的纯 Rust、单语言、个人全局 CLI 聚合与速查工具。

这是一项实际开发任务，不是只生成计划。必须先检查现有代码、工作区状态和可行性，再按阶段实施、测试和交付。不得修改与 Help Desk 无关的用户代码，不得清理或覆盖现有未提交改动。

## 一、产品目标

工具最终通过以下命令使用：

```bash
hdesk <command>
hdesk show <command>
hdesk list
hdesk search <keyword>
hdesk add <command>
hdesk edit <command>
hdesk remove <command>
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
hdesk doctor
hdesk --version
```

其目标是：发现用户真实可用的全局 CLI 和 Zsh 自定义命令，记录提供这些命令的全局包版本，生成可审查清单，由外部 AI 补全简短说明、语法和常用示例，再安全导入本地目录供快速查询。以后由用户按需手动复扫，通过版本与命令集合差异，只复核和更新受影响条目。

## 二、不可改变的范围决定

1. 使用 Rust，不保留 Python 作为正式入口或 fallback。
2. v0.1 只维护一套 UTF-8 内容，不做多语言字段或语言切换。
3. 不做 TUI；以稳定、脚本友好的 CLI 输出为主。
4. 当前没有正式数据，不实现旧 Python 数据库恢复或迁移。
5. 不读取、修改或删除旧的 `~/.help_desk.db`。
6. 不集成在线 AI API；只生成供外部 AI 使用的 JSON。
7. 不扫描项目依赖，只扫描 global/user-level 安装。
8. 扫描是只读操作，不安装、升级、删除或执行发现到的工具。
9. 不得盲目对所有命令运行 `--help`。
10. Go 扫描器默认 Deferred；不要为了“完整”擅自扩大 v0.1。
11. 产品版本为 0.1，Cargo version 使用 `0.1.0`，Git tag 使用 `v0.1.0`。
12. 首发平台只有：
    - `x86_64-unknown-linux-musl`
    - `aarch64-apple-darwin`
13. 不实现 daemon、cron、登录触发或每日轮询。扫描、diff 和 AI 更新完全由用户手动发起。
14. 包或命令从新快照消失时，不自动删除目录记录；只能标记为待用户确认。

## 三、开始前检查

在任何修改前：

1. 读取仓库约束、`REQUIREMENTS.md`、`docs/help-desk-v0.1-spec.md`、Cargo 配置和 Help Desk 相关脚本。
2. 运行 `git status --short`，保留用户现有改动。
3. 记录当前 Python 与 Rust 入口、数据库位置、CLI 行为、测试状态和发布配置。
4. 用临时 `CARGO_TARGET_DIR` 运行：

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

5. 将已有失败、新增失败和环境缺失分别记录，不得混为一谈。
6. 如果现有依赖不适合 musl 或 Apple ARM64，先用最小实验验证，再决定替代方案。

## 四、目标架构

将职责拆分为可独立测试的模块，不把所有逻辑继续放在 `main.rs`：

```text
cli          参数、输出和退出码
catalog      命令目录与查询
storage      新数据库、事务和路径
inventory    发现清单模型
snapshot     不可变环境快照、完整性与保留策略
diff         包版本、命令集合和定义指纹比较
aggregate    去重、冲突和 effective resolution
redact       AI 导出脱敏
schema       JSON 校验
sources/
  path
  zsh
  python
  cargo
  npm
  composer
release      仅放构建元数据，不含运行时发布逻辑
```

具体文件名可遵循仓库现有风格，但职责边界和测试能力必须保留。

推荐使用成熟的 CLI 参数解析库，确保未知参数返回非零退出码，并自动生成一致的 `--help`。不要为了减少一个小依赖继续维护脆弱的手写参数分派。

## 五、新数据模型

目录记录至少包含：

```text
command_id
name
kind
summary
syntax
examples[]
notes[]
tags[]
provider ecosystem/package/version
documented_against_version
source evidence
updated_at
review_state
```

不要继续新增 `*_ja`、`*_en` 字段。

数据位置：

- Linux：`${XDG_DATA_HOME:-~/.local/share}/hdesk/catalog.db`
- macOS：`~/Library/Application Support/hdesk/catalog.db`

配置位置遵循对应平台标准目录。必须允许测试通过环境变量覆盖数据和配置目录，所有自动测试不得污染真实 Home。

## 六、扫描器实现要求

### 1. PATH/user binary

默认只扫描用户和全局工具目录：`~/.local/bin`、`/usr/local/bin`、Homebrew prefix、各语言工具的 global bin，以及用户显式配置目录。不要默认枚举全部 `/usr/bin`。

只检查文件元数据、可执行位和符号链接；不得执行候选二进制。

### 2. Zsh alias/function

独立 Rust 进程无法直接读取父 Zsh 会话状态。实现 `hdesk shell-init zsh`，输出一段短小、可审查的 Shell 集成代码；用户显式加载后，由当前 Zsh 会话采集 alias、function 名称和来源元数据，并通过 stdin 交给 Rust。集成代码不得执行被发现的 alias/function。

使用 Zsh 可提供的来源信息过滤 function，仅保留来自用户配置目录的定义。排除框架、插件、补全系统和系统函数。不得默认运行 `zsh -lic` 加载真实 `.zshrc`，因为启动文件可能产生任意副作用。只有用户显式传入 `--allow-source <file>` 时才允许在隔离子 Shell 中 source 指定文件，并必须显示风险提示。

必须处理：

- 同名 alias/function/binary；
- alias 展开中的引号与管道；
- 配置加载失败；
- 定义中可能存在的秘密。

不要在测试中加载开发者真实 `.zshrc`；使用 fixture HOME 和 fixture Zsh 配置。

### 3. Python

支持：

- `pipx list --output json`；
- uv 持久安装的 tools；
- 非 virtualenv Python 的 `importlib.metadata` `console_scripts`；
- user site-packages 的 console scripts。

只收录真正暴露 CLI entry point 的包。排除项目 `.venv`、临时 `uvx` cache 和普通 library-only package。

### 4. Cargo

解析 `cargo install --list`，记录 crate、版本和它提供的所有 binary，并验证 install root 中的 executable 存在。

### 5. npm

读取 global prefix/root、顶层 global packages 和各包 `package.json` 的 `bin` 字段，只收录实际存在的 global bin。不得访问项目 `node_modules/.bin`。

### 6. Composer

获取 Composer global home 与：

```bash
composer global config bin-dir --absolute
```

只收录 global bin-dir 中的实际 CLI，并关联 global package。不得扫描项目 `vendor/bin`。

### 7. 适配器失败规则

某一生态工具未安装时输出 `skipped` 状态，不得使整个 scan 失败。解析失败必须包含 source、command、exit status 和可操作信息，但不得泄露敏感环境变量。

所有外部命令必须设置超时、关闭交互式 stdin，并用明确 argv 调用，禁止通过拼接 Shell 字符串执行不可信内容。

### 8. 包版本、快照与手动复扫

每个适配器必须把“实际提供全局 CLI 的包”作为一等 package fact，记录其能够可靠取得的原始包版本和命令集合：Python distribution/pipx/uv tool、Cargo crate、npm global package、Composer global package，以及可可靠归属的 apt/dnf/rpm/Homebrew 包。无法归属的 PATH 二进制保存路径和文件指纹，版本为 null，不得猜测。目录另存 `documented_against_version`，不能用新扫描版本覆盖说明最后一次确认时所依据的版本。

Zsh alias/function 没有包版本，改用脱敏定义指纹、来源文件和可获得的修改时间识别变化。不得把完整敏感函数体写入外发文件。

首次 `scan --snapshot` 建立基线，之后只有用户手动执行才创建新快照。快照必须保存各适配器状态和完整性；某适配器失败时，diff 不得把该来源缺失项误判为 removed。快照默认保留，清理必须通过显式、可确认的 prune 操作。

diff 状态至少覆盖 `unchanged`、`added`、`removed`、`upgraded`、`downgraded`、`version_changed`、`provider_changed`、`commands_changed` 和 `definition_changed`。不要对所有生态套用 SemVer；只有适配器提供可靠的生态版本排序时才使用 upgraded/downgraded，否则使用 version_changed。

包版本变化只产生 `needs_review`，不能证明 CLI 用法已经变化。只有命令集合差异、用户批准的 help excerpt、发布说明或其他明确证据才能支持改写说明。命令消失时保留目录条目并标记 `stale_pending_confirmation`，不得自动删除。

## 七、聚合规则

1. 包名与命令名分开，一个包可以提供多个命令。
2. 同名命令可以有多个来源候选，不得直接覆盖。
3. 保存每个候选的来源证据和置信度。
4. 使用受控 Zsh 解析结果标记当前 effective command。
5. 同一路径被不同适配器发现时合并 evidence。
6. 重复扫描输出必须稳定、排序确定且幂等。
7. 不允许通过路径字符串猜测高置信度包来源。
8. 相同完整环境的重复快照必须产生空 diff。

## 八、AI 交换安全

实现两种不同 JSON：

1. `inventory-v0.1.json`：本地事实快照；
2. `ai-input-v0.1.json`：经过最小化和脱敏、可发给外部 AI 的内容。

脱敏至少覆盖：

- 常见 API key/token/password 格式；
- URL 中的 userinfo 或 query secret；
- 私有 SSH key；
- npm/PyPI/Composer 私有 registry credential；
- 用户未批准的完整 Zsh function body；
- 无关绝对路径。

发现疑似秘密时宁可移除字段并输出 warning，不得把原文写入 AI 输入。

AI 返回的 `catalog-v0.1.json` 必须经过严格 Schema 校验。command_id 必须能关联到 inventory；不得允许 AI 凭空添加本机不存在的命令并冒充扫描事实。

`export-ai --changes-only` 只输出新增或 `needs_review` 的命令，并包含旧/新包版本、旧/新命令集合、变化原因、现有目录内容以及用户批准的证据。未变化命令不得进入更新批次。证据不足时 AI 必须返回“保持原说明/需要人工复核”，不得编造版本变化内容。

导入必须先全量验证，再使用单个事务提交。任一记录失败时数据库保持不变，并输出具体 JSON path。增量导入只能修改批次明确列出的 command_id，批次外条目必须保持字节级等价的业务内容。

## 九、测试要求

不要再接受“cargo test 成功但 0 个测试”。至少实现：

### 单元测试

- 命令名、provider 和 command_id 规范化；
- 聚合去重与同名冲突；
- effective resolution；
- 脱敏规则；
- Schema 正常与异常样例；
- 导入事务回滚；
- 确定性排序。
- 生态版本比较与未知排序回退；
- 快照完整性、空 diff 和 alias/function 指纹变化；
- changes-only 选择与定向导入边界。

### fixture/集成测试

- Zsh alias 和用户 function；
- 框架/插件 function 被排除；
- Python console_scripts 与 library-only package；
- pipx JSON；
- Cargo install list；
- npm package `bin` 字符串和对象两种格式；
- Composer global bin；
- 项目 `.venv/node_modules/vendor/target` 不进入清单；
- 某个生态工具缺失时其余适配器继续工作。
- 适配器失败不会产生伪 removed；
- 包版本变化、命令新增/移除及 provider 变化；
- 命令消失时目录记录保留并等待确认。

### CLI smoke test

在临时 HOME/data dir 中完成：

```text
empty list
add
show
search
scan fixture
snapshot list
diff unchanged
diff changed
export-ai
export-ai changes-only
validate
import
remove with confirmation
unknown option failure
```

所有 destructive catalog 操作都要验证目标；测试不得触碰真实包或 Shell 配置。

## 十、现有代码必须处理的问题

实施时需要明确解决，而不是绕过：

- Python 仍是部分安装与 `sys_update.sh` 的默认入口；
- 当前 Rust CLI 在解析 `--help` 前就打开数据库；
- 当前是多语言数据模型；
- 当前 TUI 和 AI Prompt 不是 v0.1 范围；
- 当前严格 Clippy 失败；
- 当前没有 Rust 测试；
- 当前没有 Release workflow、checksums、GPG 或 attestation；
- 当前包扫描依赖工作目录相对路径；
- 现有错误处理存在静默跳过和部分成功风险。

不要仅仅隐藏这些路径；删除正式入口后仍需保留清晰的迁移说明，避免旧脚本继续误调用 Python。

## 十一、发布与供应链

### 构建矩阵

```text
ubuntu-24.04 → x86_64-unknown-linux-musl
macos-15     → aarch64-apple-darwin
```

产物：

```text
hdesk-v0.1.0-x86_64-unknown-linux-musl.tar.gz
hdesk-v0.1.0-aarch64-apple-darwin.tar.gz
SHA256SUMS
SHA256SUMS.asc
release-key.asc
```

### CI 安全

- 固定 Rust toolchain 版本并提交 lockfile。
- 构建使用 `--locked`。
- 第三方 GitHub Actions 固定到完整 commit SHA，并在注释中标明对应版本。
- build/test job 默认只有 `contents: read`。
- attestation job 仅增加必要的 `id-token: write` 与 `attestations: write`。
- release job 单独授予 `contents: write`，并使用 protected environment/manual approval。
- tag `v0.1.0` 必须与 Cargo version `0.1.0` 一致。
- Release 先为 draft。

### GPG

推荐 CI 只构建和 attest；由维护者下载并验证产物，在本地使用离线或硬件保护的 release signing subkey 签署 `SHA256SUMS`，然后上传 `SHA256SUMS.asc` 并发布 draft。

不得把长期 GPG 主私钥放进 GitHub Actions secret。公钥指纹必须在 README 和独立可信位置公布。

### 平台验证

Linux：

```bash
file hdesk
ldd hdesk
./hdesk --version
./hdesk --help
```

确认架构为 x86_64，并且没有非预期 glibc 依赖。

macOS：

```bash
file hdesk
otool -L hdesk
./hdesk --version
./hdesk --help
```

确认架构为 arm64。Apple Developer ID/notarization 可作为后续发布门禁，但不得用它替代 GPG 与 checksum。

## 十二、实施阶段与停止点

### Phase 1：CLI 与新目录

完成单语言 Rust CLI、新数据路径、查询维护和测试。验证通过后再进入下一阶段。

### Phase 2：扫描与聚合

完成 PATH/Zsh/Python/Cargo/npm/Composer 适配器、包版本事实、inventory、不可变快照、完整性状态、diff 和聚合测试。Go 不在本阶段强制范围。

### Phase 3：AI 交换

完成脱敏、Schema、validate、changes-only 导出和只更新指定 command_id 的事务导入。

### Phase 4：发布

完成两平台构建、GitHub draft release、checksums、attestation、GPG 操作说明和验收。

不得在某阶段仍有失败测试时宣称该阶段完成。每阶段都要给出：

- 修改内容；
- 用户可观察行为；
- 实际运行的验证命令与结果；
- 未验证部分；
- 当前 Git diff；
- 下一阶段是否已满足前置条件。

## 十三、最终完成标准

只有以下全部满足才能标记 v0.1.0 完成：

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

并且：

- 两个平台 release 二进制均通过真实平台 smoke test；
- 扫描不包含项目依赖；
- 不泄露 fixture 中注入的秘密；
- 损坏 AI JSON 不产生部分导入；
- 相同环境复扫产生空 diff，部分扫描失败不产生伪删除；
- 包版本变化不会在无证据时自动改写说明；
- changes-only 导入不改变批次外目录条目；
- Release assets 的 SHA-256 验证通过；
- GPG detached signature 验证通过且 fingerprint 正确；
- GitHub artifact attestation 验证通过；
- README 提供安装、扫描、AI 协作、导入和验证步骤；
- GitHub Release tag 为 `v0.1.0`。

完成后交付代码审查摘要，按严重程度列出仍存在的风险；如果没有阻塞问题，也要明确列出未覆盖平台和延期功能。不要创建 `v0.1.0` tag 或公开 Release，除非用户在最终验收后明确授权。
