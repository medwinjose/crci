# CRCI Android App

A minimal Jetpack Compose app that calls the `crci-core` Rust library directly through UniFFI-generated
Kotlin bindings (no HTTP server in between).

**What it does today:** `startNode` creates an in-process `NodeRuntime` with an in-memory inbox and mock
storage, and runs `process_inbox` / `run_consensus` every 500 ms. `connectPeer` only adds an address to
the peer list. The app has **no network transport**, so it does not exchange messages with other devices.
It demonstrates the FFI boundary, not a working mobile mesh node.

## Prerequisites

- Android Studio with NDK r26 or newer
- [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk) and the Rust Android targets:
  ```sh
  rustup target add aarch64-linux-android x86_64-linux-android
  ```

## Build the native library

The `.so` files are build products and are not committed. From the repository root:

```sh
cargo ndk -t arm64-v8a -t x86_64 \
  -o crci-android/app/src/main/jniLibs \
  build -p crci-core
```

## Run

1. Build the native library as above.
2. Open `crci-android/` in Android Studio.
3. Run on an emulator (x86_64) or a device (arm64-v8a).

The instrumented test `Bft038PanicBoundaryTest` checks that a Rust panic surfaces as a Kotlin exception.
Its last recorded pass is in `docs/evidence/android-bft038-2026-08-23/`.

## Regenerate the Kotlin bindings

After changing the exported API in `crates/crci-core/src/ffi.rs`, run from the repository root:

```bash
bash bindings/kotlin/generate.sh
```

This writes `app/src/main/kotlin/dev/crci/android/ffi/CrciCore.kt` (package `dev.crci.android.ffi`, set
in `crates/crci-core/uniffi.toml`).
