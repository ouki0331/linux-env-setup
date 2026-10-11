#!/bin/bash

echo "Starting Linux/WSL environment restore..."

# 1. Detect package manager and install packages across distros
if [ -f "packages.txt" ]; then
    echo "Reading packages.txt..."
    PACKAGES=$(grep -vE "^\s*#" packages.txt | tr '\n' ' ')
    
    if [ -n "$PACKAGES" ]; then
        if command -v apt-get &> /dev/null; then
            echo "Detected apt-get (Ubuntu/Debian), installing..."
            sudo apt-get update
            sudo apt-get install -y $PACKAGES
        elif command -v dnf &> /dev/null; then
            echo "Detected dnf (Fedora/CentOS/RHEL/AlmaLinux), installing..."
            sudo dnf install -y $PACKAGES
        elif command -v pacman &> /dev/null; then
            echo "Detected pacman (Arch Linux/Manjaro), installing..."
            sudo pacman -Sy --noconfirm $PACKAGES
        elif command -v zypper &> /dev/null; then
            echo "Detected zypper (openSUSE), installing..."
            sudo zypper install -y $PACKAGES
        elif command -v apk &> /dev/null; then
            echo "Detected apk (Alpine Linux), installing..."
            sudo apk add $PACKAGES
        else
            echo "No supported package manager found. Please manually install packages from packages.txt."
        fi
    fi
else
    echo "packages.txt not found, skipping package installation."
fi

# 2. Restore VSCode extensions
if command -v code &> /dev/null; then
    if [ -f "vscode-extensions.txt" ]; then
        echo "Restoring VSCode extensions..."
        cat vscode-extensions.txt | xargs -L 1 code --install-extension
    else
        echo "vscode-extensions.txt not found, skipping."
    fi
else
    echo "VSCode CLI (code) not found, skipping extensions."
fi

# 3. Restore Dotfiles
echo "Restoring dotfiles..."
if [ -d "dotfiles" ]; then
    for file in dotfiles/.*; do
        filename=$(basename "$file")
        if [ "$filename" != "." ] && [ "$filename" != ".." ]; then
            cp "$file" ~/"$filename"
            echo "Restored ~/$filename"
        fi
    done
else
    echo "dotfiles directory not found, skipping dotfiles restore."
fi

# 4. Install custom CLI productivity tools to ~/.local/bin
echo "Installing custom CLI productivity tools to ~/.local/bin..."
mkdir -p ~/.local/bin
[ -f "http_cli.py" ] && cp http_cli.py ~/.local/bin/http_cli.py && chmod +x ~/.local/bin/http_cli.py && ln -sf ~/.local/bin/http_cli.py ~/.local/bin/req && echo "Installed global CLI tool: req (http_cli.py)"
[ -f "deploy.py" ] && cp deploy.py ~/.local/bin/deploy.py && chmod +x ~/.local/bin/deploy.py && echo "Installed deploy tool: deploy.py"
[ -f "rollback.py" ] && cp rollback.py ~/.local/bin/rollback.py && chmod +x ~/.local/bin/rollback.py && echo "Installed rollback tool: rollback.py"
[ -f "help_desk.py" ] && cp help_desk.py ~/.local/bin/help_desk.py && chmod +x ~/.local/bin/help_desk.py && ln -sf ~/.local/bin/help_desk.py ~/.local/bin/help-desk && ln -sf ~/.local/bin/help_desk.py ~/.local/bin/hdesk && echo "Installed Help Desk tool: help-desk (hdesk)"
[ -f "sys_update.sh" ] && cp sys_update.sh ~/.local/bin/sys_update.sh && chmod +x ~/.local/bin/sys_update.sh && ln -sf ~/.local/bin/sys_update.sh ~/.local/bin/sys-update && echo "Installed update tool: sys-update (sys_update.sh)"
[ -f "help_data_ja.json" ] && cp help_data_ja.json ~/.local/bin/help_data_ja.json && echo "Copied initial seed Help Desk catalog: help_data_ja.json"

# 5. Terminal HTML preview engine verification
if command -v w3m &> /dev/null; then
    echo "Terminal HTML renderer: w3m detected (ready for http_cli/req)."
else
    echo "Notice: w3m is recommended for terminal webpage previewing. Please install w3m if needed."
fi

echo "========================================"
echo "Environment restore completed! Please restart terminal to apply new configurations."
