# ==========================================
# Personal ZSH Configuration (~/.zshrc)
# ==========================================

# 1. PATH environment variables
export PATH="$HOME/.local/bin:$HOME/bin:$PATH"

# 2. Command aliases
alias ll="eza -la --icons"
alias ls="eza --icons"
alias cat="bat"
alias py="python3"

# 3. Custom developer workflow tools
# Interactive CLI HTTP Client & Hub
alias req="python3 ~/.local/bin/http_cli.py"
alias http-cli="python3 ~/.local/bin/http_cli.py"

# Legacy server safe pull-backup & deploy tools
alias deploy="python3 ~/.local/bin/deploy.py"
alias deploy-rollback="python3 ~/.local/bin/rollback.py"

# 4. Git shortcuts
alias gs="git status"
alias ga="git add"
alias gc="git commit -m"
alias gp="git push"
alias gl="git log --oneline --graph"

# 5. Language toolchains (NVM, Go, Rust)
[ -s "$HOME/.nvm/nvm.sh" ] && \. "$HOME/.nvm/nvm.sh"
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
export GOPATH="$HOME/go"
export PATH="$PATH:$GOPATH/bin"
