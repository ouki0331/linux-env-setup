# ==========================================
# 个人 ZSH 配置 (~/.zshrc)
# ==========================================

# 1. PATH 环境变量设置 (包含本地用户二进制目录)
export PATH="$HOME/.local/bin:$HOME/bin:$PATH"

# 2. 常用命令行别名 (Aliases)
alias ll="eza -la --icons"
alias ls="eza --icons"
alias cat="bat"
alias py="python3"

# 3. 极客工作流与自定义工具别名
# 交互式向导型 HTTP 调试与项目管理工具
alias req="python3 ~/.local/bin/http_cli.py"
alias http-cli="python3 ~/.local/bin/http_cli.py"

# 老旧服务器拉取式备份与安全发布工具
alias deploy="python3 ~/.local/bin/deploy.py"
alias deploy-rollback="python3 ~/.local/bin/rollback.py"

# 4. Git 常用快捷方式
alias gs="git status"
alias ga="git add"
alias gc="git commit -m"
alias gp="git push"
alias gl="git log --oneline --graph"

# 5. 语言环境加载 (NVM, Go, Rust 等按需自动适配)
[ -s "$HOME/.nvm/nvm.sh" ] && \. "$HOME/.nvm/nvm.sh"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
export GOPATH="$HOME/go"
export PATH="$PATH:$GOPATH/bin"
