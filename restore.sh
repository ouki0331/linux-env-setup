#!/bin/bash

echo "开始恢复 Linux/WSL 工作环境..."

# 1. 自动检测包管理器并安装跨发行版的通用软件
if [ -f "packages.txt" ]; then
    echo "正在读取 packages.txt..."
    # 读取文件，忽略注释行和空行
    PACKAGES=$(grep -vE "^\s*#" packages.txt | tr '\n' ' ')
    
    if [ -n "$PACKAGES" ]; then
        if command -v apt-get &> /dev/null; then
            echo "检测到 apt-get (Ubuntu/Debian)，开始安装..."
            sudo apt-get update
            sudo apt-get install -y $PACKAGES
        elif command -v dnf &> /dev/null; then
            echo "检测到 dnf (Fedora/CentOS/RHEL)，开始安装..."
            sudo dnf install -y $PACKAGES
        elif command -v pacman &> /dev/null; then
            echo "检测到 pacman (Arch Linux/Manjaro)，开始安装..."
            sudo pacman -Sy --noconfirm $PACKAGES
        elif command -v zypper &> /dev/null; then
            echo "检测到 zypper (openSUSE)，开始安装..."
            sudo zypper install -y $PACKAGES
        elif command -v apk &> /dev/null; then
            echo "检测到 apk (Alpine Linux)，开始安装..."
            sudo apk add $PACKAGES
        else
            echo "未找到支持的包管理器，请手动安装 packages.txt 中的包。"
        fi
    fi
else
    echo "未找到 packages.txt，跳过系统软件安装。"
fi

# 2. 恢复 VSCode 扩展
if command -v code &> /dev/null; then
    if [ -f "vscode-extensions.txt" ]; then
        echo "正在恢复 VSCode 扩展..."
        cat vscode-extensions.txt | xargs -L 1 code --install-extension
    else
        echo "未找到 vscode-extensions.txt，跳过。"
    fi
else
    echo "未检测到 VSCode cli，跳过扩展安装。"
fi

# 3. 恢复配置文件 (Dotfiles)
echo "正在恢复配置文件..."
if [ -d "dotfiles" ]; then
    # 遍历 dotfiles 目录下的隐藏文件并复制到 ~ 目录
    for file in dotfiles/.*; do
        # 排除 . 和 .. 目录
        filename=$(basename "$file")
        if [ "$filename" != "." ] && [ "$filename" != ".." ]; then
            cp "$file" ~/"$filename"
            echo "已恢复 ~/$filename"
        fi
    done
else
    echo "未找到 dotfiles 目录，跳过配置文件的恢复。"
fi

echo "========================================"
echo "工作环境恢复完成！部分配置可能需要重新启动终端才能生效。"
