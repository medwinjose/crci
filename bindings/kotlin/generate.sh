#!/usr/bin/env bash
set -euo pipefail

# Build the crci-core library
cargo build -p crci-core

# Determine the library extension based on the OS
LIB_EXT="so"
if [[ "$(uname -s)" == "Darwin" ]]; then
    LIB_EXT="dylib"
elif [[ "$(uname -s)" == *"MINGW"* ]] || [[ "$(uname -s)" == *"CYGWIN"* ]]; then
    LIB_EXT="dll"
fi

# Run the uniffi-bindgen binary
cargo run -p crci-core --bin uniffi-bindgen -- \
  generate \
  --library target/debug/libcrci_core.${LIB_EXT} \
  --language kotlin \
  --out-dir bindings/kotlin/

echo "Kotlin bindings written to bindings/kotlin/"
# Note: Windows users should run this script inside WSL or Git Bash,
# or use cross-compile via the RPi pipeline.
