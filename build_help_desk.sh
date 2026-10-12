#!/bin/bash
# ==============================================================================
# Help Desk Build Script
# Automatically builds the Rust TUI application.
# If cargo is not available on the host system, it pulls the official Rust
# environment image (rust:slim) from Docker Hub and compiles inside a container.
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/crates/help-desk/target/release"
BINARY_PATH="$TARGET_DIR/help-desk"

echo "============================================================"
echo "🦀 Building Help Desk (Rust TUI)..."
echo "============================================================"

if command -v cargo &> /dev/null; then
    echo "Host Cargo detected: $(cargo --version)"
    echo "Compiling natively via cargo..."
    cargo build --release --manifest-path "$SCRIPT_DIR/crates/help-desk/Cargo.toml"
    echo "Native compilation completed: $BINARY_PATH"
elif command -v docker &> /dev/null; then
    echo "Cargo not detected on host. Docker is available: $(docker --version)"
    echo "Pulling official Rust runtime image (rust:slim) from Docker Hub..."
    docker pull rust:slim

    echo "Creating builder container from rust:slim..."
    CID=$(docker create -w /build rust:slim sh -c "cargo build --release --manifest-path crates/help-desk/Cargo.toml")

    echo "Preparing source files..."
    TEMP_DIR=$(mktemp -d)
    mkdir -p "$TEMP_DIR/crates/help-desk"
    cp "$SCRIPT_DIR/crates/help-desk/Cargo.toml" "$TEMP_DIR/crates/help-desk/"
    if [ -f "$SCRIPT_DIR/crates/help-desk/Cargo.lock" ]; then
        cp "$SCRIPT_DIR/crates/help-desk/Cargo.lock" "$TEMP_DIR/crates/help-desk/"
    fi
    cp -r "$SCRIPT_DIR/crates/help-desk/src" "$TEMP_DIR/crates/help-desk/"

    echo "Copying source files into container..."
    docker cp "$TEMP_DIR/crates" "$CID:/build/"
    rm -rf "$TEMP_DIR"

    echo "Compiling Help Desk inside Docker container..."
    docker start -a "$CID"

    echo "Extracting release binary from container..."
    mkdir -p "$TARGET_DIR"
    docker cp "$CID:/build/crates/help-desk/target/release/help-desk" "$BINARY_PATH"
    chmod +x "$BINARY_PATH"

    echo "Cleaning up builder container..."
    docker rm "$CID" > /dev/null
    echo "Container compilation completed: $BINARY_PATH"
else
    echo "Neither Cargo nor Docker was found on this system."
    echo "To compile native Help Desk, install Rust (cargo) or Docker."
    echo "Fallback Python implementation (help_desk.py) will remain active."
    exit 1
fi

echo "============================================================"
echo "🎉 Help Desk binary is ready at: $BINARY_PATH"
echo "============================================================"
