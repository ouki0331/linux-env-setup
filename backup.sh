#!/bin/bash

echo "Starting Linux/WSL environment backup..."

# 1. Backup VSCode extensions
if command -v code &> /dev/null; then
    echo "Backing up VSCode extensions..."
    code --list-extensions > vscode-extensions.txt
    echo "VSCode extensions backed up to vscode-extensions.txt."
else
    echo "VSCode CLI (code) not found, skipping."
fi

# 2. Backup Dotfiles
echo "Backing up dotfiles..."
mkdir -p dotfiles
for file in ~/.bashrc ~/.zshrc ~/.gitconfig ~/.tmux.conf ~/.vimrc ~/.profile; do
    if [ -f "$file" ]; then
        cp "$file" dotfiles/
        echo "Backed up $file"
    fi
done

echo "========================================"
echo "Backup complete!"
echo "Check packages.txt to ensure your tool list is updated, then git commit."
