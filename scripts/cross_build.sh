#!/bin/bash
set -e

usage() {
    echo "Usage: $0 [armv7|aarch64|all]"
    echo "  armv7    - Build for Raspberry Pi 2/3/4 (32-bit)"
    echo "  aarch64  - Build for Raspberry Pi 4/5 (64-bit)"
    echo "  all      - Build for both targets (default)"
    exit 1
}

build_target() {
    local target_name=$1
    local triple=""

    if [ "$target_name" == "armv7" ]; then
        triple="armv7-unknown-linux-gnueabihf"
    elif [ "$target_name" == "aarch64" ]; then
        triple="aarch64-unknown-linux-gnu"
    else
        usage
    fi

    echo "========================================"
    echo "Building CRCI for $target_name ($triple)"
    echo "========================================"

    rustup target add "$triple"
    cargo build --release --target "$triple"

    local bin_path="target/$triple/release/crci"
    echo ""
    echo "Build successful! Binary location and size:"
    ls -lh "$bin_path"
    echo ""
}

TARGET=${1:-all}

if [ "$TARGET" == "all" ]; then
    build_target "armv7"
    build_target "aarch64"
elif [ "$TARGET" == "armv7" ] || [ "$TARGET" == "aarch64" ]; then
    build_target "$TARGET"
else
    usage
fi
