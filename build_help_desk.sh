#!/bin/bash
# ==============================================================================
# Help Desk Build Script (v0.1.0)
# Builds the official hdesk Rust CLI binary.
# If cargo is not available on the host system, it pulls the official Rust
# environment image (rust:slim) from Docker Hub and compiles inside a container.
# Note: Rust is the sole official implementation; Python is not used as fallback.
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/crates/help-desk/target/release"
BINARY_PATH="$TARGET_DIR/hdesk"
SYMLINK_PATH="$TARGET_DIR/help-desk"

echo "============================================================"
echo "🦀 Building Personal CLI Help Desk (hdesk v0.1.0)..."
echo "============================================================"

if command -v cargo &> /dev/null; then
    echo "Host Cargo detected: $(cargo --version)"
    echo "Compiling natively via cargo..."
    cargo build --release --manifest-path "$SCRIPT_DIR/crates/help-desk/Cargo.toml"
    ln -sf "$BINARY_PATH" "$SYMLINK_PATH"
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
    if [ -f "$SCRIPT_DIR/crates/help-desk/build.rs" ]; then
        cp "$SCRIPT_DIR/crates/help-desk/build.rs" "$TEMP_DIR/crates/help-desk/"
    fi

    echo "Copying source files into container..."
    docker cp "$TEMP_DIR/crates" "$CID:/build/"
    rm -rf "$TEMP_DIR"

    echo "Compiling Help Desk inside Docker container..."
    docker start -a "$CID"

    echo "Extracting release binary from container..."
    mkdir -p "$TARGET_DIR"
    docker cp "$CID:/build/crates/help-desk/target/release/hdesk" "$BINARY_PATH"
    chmod +x "$BINARY_PATH"
    ln -sf "$BINARY_PATH" "$SYMLINK_PATH"

    echo "Cleaning up builder container..."
    docker rm "$CID" > /dev/null
    echo "Container compilation completed: $BINARY_PATH"
else
    echo "Error: Neither Cargo nor Docker was found on this system."
    echo "To compile native Help Desk, install Rust (cargo) or Docker."
    echo "Python fallback has been deprecated in v0.1 as per specification."
    exit 1
fi

echo "============================================================"
echo "🎉 Help Desk binary is ready at: $BINARY_PATH (and $SYMLINK_PATH)"
echo "============================================================"
