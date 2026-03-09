#!/bin/sh
set -e

REPO="anthropics/memory-agent"
VERSION="${1:-$(curl -sS "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)}"

if [ -z "$VERSION" ]; then
    echo "Error: Could not determine latest version." >&2
    exit 1
fi

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    arm64|aarch64) ARCH="aarch64" ;;
    *)
        echo "Error: Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

BINARY="memory-agent-${ARCH}-${OS}"
URL="https://github.com/$REPO/releases/download/$VERSION/$BINARY"

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"

echo "Downloading memory-agent $VERSION for $ARCH-$OS..."
if ! curl -fsSL "$URL" -o "$INSTALL_DIR/memory-agent"; then
    echo "Error: Download failed. Check that $VERSION exists for $ARCH-$OS." >&2
    exit 1
fi
chmod +x "$INSTALL_DIR/memory-agent"

echo "memory-agent $VERSION installed to $INSTALL_DIR/memory-agent"

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Add $INSTALL_DIR to your PATH if not already present."
fi
