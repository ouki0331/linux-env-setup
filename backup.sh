#!/bin/bash

echo "开始备份 Linux/WSL 工作环境..."

# 1. 备份 VSCode 扩展
# WSL 环境下如果连接了 VSCode，也可以直接使用 code 命令
if command -v code &> /dev/null; then
    echo "备份 VSCode 扩展..."
    code --list-extensions > vscode-extensions.txt
    echo "VSCode 扩展备份完成 (已生成 vscode-extensions.txt)。"
else
    echo "未检测到 VSCode cli (code 命令)，跳过。"
fi

# 2. 备份配置文件 (Dotfiles)
echo "备份配置文件..."
mkdir -p dotfiles
# 常见的 Linux 配置文件列表，你可以根据需要随时追加
for file in ~/.bashrc ~/.zshrc ~/.gitconfig ~/.tmux.conf ~/.vimrc ~/.profile; do
    if [ -f "$file" ]; then
        cp "$file" dotfiles/
        echo "已备份 $file"
    fi
done

echo "========================================"
echo "备份完成！"
echo "请记得检查 packages.txt 中是否需要手动添加新工具的名字。"
echo "最后使用 git commit 提交所有更改即可。"
