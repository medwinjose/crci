#!/usr/bin/env bash
# Regenerates the Kotlin bindings used by crci-android.
# Run from the repository root (Linux, macOS, WSL or Git Bash).
set -euo pipefail

OUT_FILE="crci-android/app/src/main/kotlin/dev/crci/android/ffi/CrciCore.kt"

# Build the crci-core library
cargo build -p crci-core

# Determine the library extension based on the OS
LIB_EXT="so"
if [[ "$(uname -s)" == "Darwin" ]]; then
    LIB_EXT="dylib"
elif [[ "$(uname -s)" == *"MINGW"* ]] || [[ "$(uname -s)" == *"CYGWIN"* ]]; then
    LIB_EXT="dll"
fi

LIB_PATH="target/debug/libcrci_core.${LIB_EXT}"
if [[ "${LIB_EXT}" == "dll" ]]; then
    LIB_PATH="target/debug/crci_core.dll"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

# Run the uniffi-bindgen binary (package name comes from crates/crci-core/uniffi.toml)
cargo run -p crci-core --bin uniffi-bindgen -- \
  generate \
  --library "${LIB_PATH}" \
  --language kotlin \
  --out-dir "${TMP_DIR}"

cp "${TMP_DIR}/dev/crci/android/ffi/crci_core.kt" "${OUT_FILE}"
echo "Kotlin bindings written to ${OUT_FILE}"
