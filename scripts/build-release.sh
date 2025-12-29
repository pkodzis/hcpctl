#!/bin/bash
# Build release binaries for multiple platforms
# Requires: cross (cargo install cross)
# Requires: Docker (for cross-compilation)

set -e

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
NAME="tfe-ws"
DIST_DIR="dist"

echo "Building ${NAME} v${VERSION}"
echo "=========================="

# Create dist directory
mkdir -p "${DIST_DIR}"

# Define targets
TARGETS=(
    # Linux
    "x86_64-unknown-linux-gnu"        # Linux x86_64 (most common)
    "x86_64-unknown-linux-musl"       # Linux x86_64 static (Alpine, containers)
    "aarch64-unknown-linux-gnu"       # Linux ARM64 (AWS Graviton, Apple M1 VMs)

    # macOS
    "x86_64-apple-darwin"             # macOS Intel
    "aarch64-apple-darwin"            # macOS Apple Silicon (M1/M2/M3)

    # Windows
    "x86_64-pc-windows-gnu"           # Windows x86_64
)

# Build for each target
for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "Building for ${TARGET}..."

    # Use cross for cross-compilation, cargo for native
    if [[ "${TARGET}" == *"darwin"* ]]; then
        # macOS targets need actual macOS or special setup
        echo "  ⚠️  Skipping ${TARGET} (requires macOS host or osxcross)"
        continue
    fi

    if cross build --release --target "${TARGET}" 2>/dev/null; then
        # Determine binary name
        if [[ "${TARGET}" == *"windows"* ]]; then
            BINARY="${NAME}.exe"
        else
            BINARY="${NAME}"
        fi

        # Copy and rename binary
        SRC="target/${TARGET}/release/${BINARY}"
        if [[ -f "${SRC}" ]]; then
            DEST="${DIST_DIR}/${NAME}-${VERSION}-${TARGET}"
            if [[ "${TARGET}" == *"windows"* ]]; then
                DEST="${DEST}.exe"
            fi
            cp "${SRC}" "${DEST}"

            # Show size
            SIZE=$(ls -lh "${DEST}" | awk '{print $5}')
            echo "  ✅ Built: ${DEST} (${SIZE})"
        else
            echo "  ❌ Binary not found: ${SRC}"
        fi
    else
        echo "  ❌ Build failed for ${TARGET}"
    fi
done

echo ""
echo "=========================="
echo "Build complete! Binaries in ${DIST_DIR}/"
ls -lh "${DIST_DIR}/"
