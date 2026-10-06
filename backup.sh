#!/bin/bash

# 备份工作环境脚本 (Backup script)

echo "开始备份工作环境..."

# 1. 备份 Homebrew 包 (确保已安装 brew)
if command -v brew &> /dev/null; then
    echo "备份 Homebrew 软件包..."
    brew bundle dump --force --file=Brewfile
    echo "Homebrew 软件包备份完成 (已生成 Brewfile)。"
else
    echo "未检测到 Homebrew，跳过。"
fi

# 2. 备份 VSCode 扩展 (确保已安装 code 命令)
if command -v code &> /dev/null; then
    echo "备份 VSCode 扩展..."
    code --list-extensions > vscode-extensions.txt
    echo "VSCode 扩展备份完成 (已生成 vscode-extensions.txt)。"
else
    echo "未检测到 VSCode cli (code 命令)，跳过。"
fi

# 3. 备份常用配置文件 (Dotfiles)
echo "备份配置文件..."
mkdir -p dotfiles
[ -f ~/.zshrc ] && cp ~/.zshrc dotfiles/.zshrc && echo "已备份 ~/.zshrc"
[ -f ~/.gitconfig ] && cp ~/.gitconfig dotfiles/.gitconfig && echo "已备份 ~/.gitconfig"

echo "备份完成！请使用 git commit 保存更改。"
