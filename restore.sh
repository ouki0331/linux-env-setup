#!/bin/bash

# 恢复工作环境脚本 (Restore script)

echo "开始恢复工作环境..."

# 1. 安装/恢复 Homebrew 包
if ! command -v brew &> /dev/null; then
    echo "未检测到 Homebrew，正在安装..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

if [ -f "Brewfile" ]; then
    echo "使用 Brewfile 恢复 Homebrew 软件包..."
    brew bundle install --file=Brewfile
else
    echo "未找到 Brewfile，跳过。"
fi

# 2. 安装/恢复 VSCode 扩展
if command -v code &> /dev/null; then
    if [ -f "vscode-extensions.txt" ]; then
        echo "恢复 VSCode 扩展..."
        cat vscode-extensions.txt | xargs -L 1 code --install-extension
    else
        echo "未找到 vscode-extensions.txt，跳过。"
    fi
else
    echo "未检测到 VSCode cli (code 命令)，跳过扩展安装。"
fi

# 3. 恢复配置文件 (Dotfiles)
echo "恢复配置文件..."
if [ -d "dotfiles" ]; then
    [ -f "dotfiles/.zshrc" ] && cp dotfiles/.zshrc ~/.zshrc && echo "已恢复 ~/.zshrc"
    [ -f "dotfiles/.gitconfig" ] && cp dotfiles/.gitconfig ~/.gitconfig && echo "已恢复 ~/.gitconfig"
else
    echo "未找到 dotfiles 目录，跳过。"
fi

echo "工作环境恢复完成！可能需要重启终端以使配置生效。"
