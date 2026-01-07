#!/bin/bash
# hcpctl installer for Linux/macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.sh | bash

set -e

REPO="pkodzis/hcpctl"
BINARY_NAME="hcpctl"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
info_n() { echo -n -e "${GREEN}[INFO]${NC} $1"; }  # No newline
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="linux" ;;
        Darwin*) os="macos" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="amd64" ;;
        arm64|aarch64) arch="arm64" ;;
        *)             error "Unsupported architecture: $(uname -m)" ;;
    esac

    # Linux ARM64 not available, suggest musl for compatibility
    if [[ "$os" == "linux" && "$arch" == "arm64" ]]; then
        PLATFORM="linux_arm64"
    elif [[ "$os" == "linux" ]]; then
        PLATFORM="linux_amd64"
    else
        PLATFORM="${os}_${arch}"
    fi

    info "Detected platform: $PLATFORM"
}

# Get latest release version
get_latest_version() {
    VERSION=$(curl -fsSIL "https://github.com/${REPO}/releases/latest" 2>/dev/null | grep -i '^location:' | sed -E 's|.*/tag/([^[:space:]]+).*|\1|')
    if [[ -z "$VERSION" ]]; then
        error "Failed to get latest version"
    fi
    info "Latest version: $VERSION"
}

# Download and verify
download_and_verify() {
    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    cd "$tmp_dir"

    local base_url="https://github.com/${REPO}/releases/download/${VERSION}"
    local archive="${BINARY_NAME}_${VERSION}_${PLATFORM}.tar.gz"

    info "Downloading ${base_url}/${archive} ..."
    curl -fsSL -O "${base_url}/${archive}" || error "Failed to download archive"

    info "Downloading SHA256SUMS..."
    curl -fsSL -O "${base_url}/SHA256SUMS" || error "Failed to download checksums"

    # Verify checksum
    info_n "Verifying checksum... "
    if command -v sha256sum &> /dev/null; then
        grep "${archive}" SHA256SUMS | sha256sum -c - > /dev/null 2>&1 && echo "OK" || { echo "FAILED"; error "Checksum verification failed!"; }
    elif command -v shasum &> /dev/null; then
        grep "${archive}" SHA256SUMS | shasum -a 256 -c - > /dev/null 2>&1 && echo "OK" || { echo "FAILED"; error "Checksum verification failed!"; }
    else
        echo "skipped"
        warn "No sha256sum/shasum found"
    fi

    # Verify GPG signature if available
    if curl -fsSL -O "${base_url}/SHA256SUMS.sig" 2>/dev/null; then
        if command -v gpg &> /dev/null; then
            info "Downloading public key..."
            curl -fsSL -O "https://raw.githubusercontent.com/${REPO}/main/public-key.asc" 2>/dev/null || true

            if [[ -f "public-key.asc" ]]; then
                gpg --import public-key.asc 2>/dev/null || true
                info "Verifying GPG signature..."
                if gpg --verify SHA256SUMS.sig SHA256SUMS 2>/dev/null; then
                    info "GPG signature verified!"
                else
                    warn "GPG signature verification failed - proceeding anyway (checksum passed)"
                fi
            else
                warn "Public key not found, skipping GPG verification"
            fi
        else
            warn "GPG not installed, skipping signature verification"
        fi
    else
        warn "No GPG signature found for this release"
    fi

    # Extract
    info "Extracting..."
    tar -xzf "${archive}"

    # Install
    mkdir -p "$INSTALL_DIR"
    mv "${BINARY_NAME}" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/${BINARY_NAME}"

    info "Installed to: $INSTALL_DIR/${BINARY_NAME}"
}

# Check if in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        warn "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi
}

# Main
main() {
    info "Installing ${BINARY_NAME}..."
    detect_platform
    get_latest_version
    download_and_verify
    check_path
    info "Installation complete! Run '${BINARY_NAME} --version' to verify."
}

main "$@"
