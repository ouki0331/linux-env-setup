#!/usr/bin/env bash
# ==============================================================================
# System Update & Help Desk Incremental Trigger (sys_update.sh / sys-update)
# - Detects Linux distribution (AlmaLinux, CentOS, Fedora, Ubuntu, Debian, Arch, openSUSE, Alpine, macOS)
# - Inspects upgradable packages and runs system update
# - Computes incremental packages that need updated Help Desk documentation
# - Prompts user to generate AI Help Desk prompt with JSON schema ready for web AI
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HELP_DESK_BIN="${HOME}/.local/bin/help_desk.py"
[ ! -f "$HELP_DESK_BIN" ] && HELP_DESK_BIN="${SCRIPT_DIR}/help_desk.py"

echo "============================================================"
echo "🚀 Starting System Update & Help Desk Inspector"
echo "============================================================"

# 1. Detect package manager
PKG_MGR=""
if command -v dnf &>/dev/null; then
    PKG_MGR="dnf"
    DISTRO="Fedora / CentOS / AlmaLinux / RHEL"
elif command -v apt-get &>/dev/null; then
    PKG_MGR="apt"
    DISTRO="Ubuntu / Debian"
elif command -v pacman &>/dev/null; then
    PKG_MGR="pacman"
    DISTRO="Arch Linux / Manjaro"
elif command -v zypper &>/dev/null; then
    PKG_MGR="zypper"
    DISTRO="openSUSE"
elif command -v apk &>/dev/null; then
    PKG_MGR="apk"
    DISTRO="Alpine Linux"
elif command -v brew &>/dev/null; then
    PKG_MGR="brew"
    DISTRO="macOS (Homebrew)"
else
    echo "⚠️ Unknown or unsupported package manager."
    DISTRO="Generic Linux"
fi

echo "Detected Environment: $DISTRO ($PKG_MGR)"
echo "------------------------------------------------------------"

DRY_RUN=false
for arg in "$@"; do
    if [ "$arg" = "--dry-run" ] || [ "$arg" = "--check-only" ]; then
        DRY_RUN=true
    fi
done

UPDATED_PKGS=()

# 2. Check for available updates and execute upgrade
if [ "$PKG_MGR" = "dnf" ]; then
    echo "Checking available package updates via dnf..."
    set +e
    RAW_UPDATES=$(dnf check-update --quiet 2>/dev/null | awk '{print $1}' | cut -d. -f1 | grep -v '^$')
    set -e
    if [ -n "$RAW_UPDATES" ]; then
        echo "Found upgradable packages:"
        echo "$RAW_UPDATES" | sed 's/^/  • /'
        UPDATED_PKGS=($(echo "$RAW_UPDATES" | tr '\n' ' '))
        if [ "$DRY_RUN" = false ]; then
            echo "Executing: sudo dnf upgrade -y"
            sudo dnf upgrade -y || echo "⚠️ dnf upgrade encountered warnings/errors, continuing to Help Desk check."
        else
            echo "[Dry-run mode: skipped sudo dnf upgrade]"
        fi
    else
        echo "System packages are up to date."
    fi

elif [ "$PKG_MGR" = "apt" ]; then
    echo "Updating package lists via apt update..."
    if [ "$DRY_RUN" = false ]; then
        sudo apt-get update -y 2>/dev/null || true
    fi
    RAW_UPDATES=$(apt list --upgradable 2>/dev/null | grep -v 'Listing...' | cut -d/ -f1 | grep -v '^$' || true)
    if [ -n "$RAW_UPDATES" ]; then
        echo "Found upgradable packages:"
        echo "$RAW_UPDATES" | sed 's/^/  • /'
        UPDATED_PKGS=($(echo "$RAW_UPDATES" | tr '\n' ' '))
        if [ "$DRY_RUN" = false ]; then
            echo "Executing: sudo apt-get upgrade -y"
            sudo apt-get upgrade -y || echo "⚠️ apt-get upgrade encountered warnings/errors, continuing to Help Desk check."
        else
            echo "[Dry-run mode: skipped sudo apt-get upgrade]"
        fi
    else
        echo "System packages are up to date."
    fi

elif [ "$PKG_MGR" = "pacman" ]; then
    echo "Checking updates via pacman..."
    if command -v checkupdates &>/dev/null; then
        RAW_UPDATES=$(checkupdates 2>/dev/null | awk '{print $1}' || true)
    else
        RAW_UPDATES=$(pacman -Qu 2>/dev/null | awk '{print $1}' || true)
    fi
    if [ -n "$RAW_UPDATES" ]; then
        UPDATED_PKGS=($(echo "$RAW_UPDATES" | tr '\n' ' '))
        if [ "$DRY_RUN" = false ]; then
            sudo pacman -Syu --noconfirm || echo "⚠️ pacman update encountered warnings/errors."
        else
            echo "[Dry-run mode: skipped sudo pacman -Syu]"
        fi
    else
        echo "System packages are up to date."
    fi

elif [ "$PKG_MGR" = "brew" ]; then
    echo "Checking updates via brew..."
    set +e
    RAW_UPDATES=$(brew outdated --quiet 2>/dev/null || true)
    set -e
    if [ -n "$RAW_UPDATES" ]; then
        UPDATED_PKGS=($(echo "$RAW_UPDATES" | tr '\n' ' '))
        if [ "$DRY_RUN" = false ]; then
            brew upgrade 2>/dev/null || echo "⚠️ brew upgrade encountered warnings/errors, continuing to Help Desk check."
        else
            echo "[Dry-run mode: skipped brew upgrade]"
        fi
    else
        echo "Homebrew packages are up to date."
    fi
fi

# 3. Check for undocumented packages from packages.txt
UNDOC_PKGS=()
if [ -f "$HELP_DESK_BIN" ]; then
    set +e
    UNDOC_STR=$(python3 "$HELP_DESK_BIN" --diff 2>/dev/null | grep '  • ' | awk '{print $2}')
    set -e
    if [ -n "$UNDOC_STR" ]; then
        UNDOC_PKGS=($(echo "$UNDOC_STR" | tr '\n' ' '))
    fi
fi

# 4. Merge and deduplicate incremental packages
ALL_INCREMENTAL=($(printf "%s\n" "${UPDATED_PKGS[@]}" "${UNDOC_PKGS[@]}" | grep -v '^$' | sort -u))

echo "============================================================"
echo "📊 Update Inspection Summary:"
echo "  • System upgraded packages: ${#UPDATED_PKGS[@]}"
echo "  • Undocumented packages   : ${#UNDOC_PKGS[@]}"
echo "  • Total incremental items : ${#ALL_INCREMENTAL[@]}"
echo "============================================================"

if [ ${#ALL_INCREMENTAL[@]} -eq 0 ]; then
    echo "🎉 Help Desk is fully synchronized with current packages. No new updates needed."
    exit 0
fi

echo "The following incremental packages have updates or missing documentation:"
for pkg in "${ALL_INCREMENTAL[@]}"; do
    echo "  👉 $pkg"
done
echo "------------------------------------------------------------"

# 5. Prompt user whether to generate AI Help Desk prompt
read -p "Would you like to generate the AI prompt to update the Help Desk? (y/n) [default: y]: " CONFIRM_UPDATE
CONFIRM_UPDATE=${CONFIRM_UPDATE:-y}

if [[ "$CONFIRM_UPDATE" =~ ^[Yy]$ ]]; then
    PROMPT_FILE="${PWD}/help_desk_ai_prompt.txt"
    echo ""
    echo "Generating AI Help Desk prompt with schema for [${#ALL_INCREMENTAL[@]}] packages..."
    python3 "$HELP_DESK_BIN" --export-prompt "${ALL_INCREMENTAL[@]}" > "$PROMPT_FILE"

    echo "============================================================"
    echo "✅ AI Prompt successfully generated and saved to:"
    echo "   $PROMPT_FILE"
    echo "============================================================"
    cat "$PROMPT_FILE"
    echo "============================================================"
    echo "👉 Instructions to complete Help Desk update:"
    echo "  1. Copy the prompt above and paste it into ChatGPT / Claude / Gemini web interface."
    echo "  2. Save the returned JSON into a file named 'help_update.json'."
    echo "  3. Run the following command to update your Help Desk database:"
    echo "     help-desk --import help_update.json"
    echo "============================================================"
else
    echo "Skipped Help Desk update. You can run 'help-desk --export-prompt' anytime."
fi
