#!/bin/sh
set -eu

# Aether VR Engine installer
# Usage: curl -fsSL https://raw.githubusercontent.com/<org>/aether/main/install.sh | sh

REPO="<org>/aether"
INSTALL_DIR="${AETHER_INSTALL_DIR:-$HOME/.aether/bin}"

main() {
    detect_platform
    get_latest_version
    download_and_install
    print_success
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  OS_TARGET="unknown-linux-gnu" ;;
        Darwin) OS_TARGET="apple-darwin" ;;
        *)
            echo "Error: unsupported OS '$OS'. Only Linux and macOS are supported." >&2
            echo "For Windows, download the .zip from GitHub Releases." >&2
            exit 1
            ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_TARGET="x86_64" ;;
        arm64|aarch64) ARCH_TARGET="aarch64" ;;
        *)
            echo "Error: unsupported architecture '$ARCH'." >&2
            exit 1
            ;;
    esac

    TARGET="${ARCH_TARGET}-${OS_TARGET}"
    echo "Detected platform: ${TARGET}"
}

get_latest_version() {
    if [ -n "${AETHER_VERSION:-}" ]; then
        VERSION="$AETHER_VERSION"
    else
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
    fi

    if [ -z "$VERSION" ]; then
        echo "Error: could not determine latest version." >&2
        exit 1
    fi

    echo "Installing Aether ${VERSION}..."
}

download_and_install() {
    ARCHIVE="aether-${VERSION}-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT

    echo "Downloading ${URL}..."
    curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"

    echo "Extracting to ${INSTALL_DIR}..."
    mkdir -p "$INSTALL_DIR"
    tar xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

    # Copy all binaries from the extracted directory
    EXTRACTED_DIR="${TMPDIR}/aether-${VERSION}-${TARGET}"
    for bin in "$EXTRACTED_DIR"/*; do
        if [ -f "$bin" ] && [ -x "$bin" ]; then
            cp "$bin" "$INSTALL_DIR/"
        fi
    done

    chmod +x "$INSTALL_DIR"/*
}

print_success() {
    echo ""
    echo "Aether ${VERSION} installed to ${INSTALL_DIR}"
    echo ""

    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo "Add Aether to your PATH by adding this to your shell profile:"
            echo ""
            echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            echo ""
            ;;
    esac

    echo "Get started:"
    echo "  aether version"
    echo "  aether run --list"
    echo "  aether run 3d-demo"
}

main
