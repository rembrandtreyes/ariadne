#!/usr/bin/env bash
set -euo pipefail

# Ariadne installer — works on macOS and Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/loremllc/ariadne/main/install.sh | bash

REPO="loremllc/ariadne"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

main() {
    echo "Installing Ariadne — the thread through the labyrinth..."
    echo ""

    # Detect platform
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Darwin) PLATFORM="apple-darwin" ;;
        Linux)  PLATFORM="unknown-linux-gnu" ;;
        *)      echo "Error: Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH="x86_64" ;;
        arm64|aarch64) ARCH="aarch64" ;;
        *)             echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    TARGET="${ARCH}-${PLATFORM}"

    # Check if we can get a prebuilt binary
    VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/' || echo "")

    if [ -n "$VERSION" ]; then
        echo "Downloading ariadne v${VERSION} for ${TARGET}..."
        URL="https://github.com/${REPO}/releases/download/v${VERSION}/ariadne-${TARGET}.tar.gz"

        TMPDIR=$(mktemp -d)
        trap 'rm -rf "$TMPDIR"' EXIT

        if curl -fsSL "$URL" -o "$TMPDIR/ariadne.tar.gz" 2>/dev/null; then
            tar -xzf "$TMPDIR/ariadne.tar.gz" -C "$TMPDIR"
            install_binary "$TMPDIR/ariadne"
            return
        fi
        echo "No prebuilt binary found, building from source..."
    fi

    # Fallback: build from source
    build_from_source
}

build_from_source() {
    # Check for Rust
    if ! command -v cargo &>/dev/null; then
        echo "Rust not found. Installing via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Clone and build
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT

    echo "Building from source (this takes ~60s)..."
    git clone --depth 1 "https://github.com/${REPO}.git" "$TMPDIR/ariadne"
    cd "$TMPDIR/ariadne"
    cargo build --release

    install_binary "$TMPDIR/ariadne/target/release/ariadne"
}

install_binary() {
    local BIN="$1"

    if [ -w "$INSTALL_DIR" ]; then
        install -m 755 "$BIN" "$INSTALL_DIR/ariadne"
    else
        echo "Need sudo to install to $INSTALL_DIR"
        sudo install -m 755 "$BIN" "$INSTALL_DIR/ariadne"
    fi

    echo ""
    echo "✓ ariadne installed to ${INSTALL_DIR}/ariadne"
    echo ""
    echo "  Get started:"
    echo "    cd your-project"
    echo "    ariadne index ."
    echo "    ariadne stats"
    echo "    ariadne search 'login'"
    echo "    ariadne blast-radius login_user"
    echo ""
}

main
