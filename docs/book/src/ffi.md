# FFI & Mobile Integration

CRCI uses Mozilla's `uniffi` to generate a robust Kotlin interface, enabling seamless integration into Android apps.

## Architecture

The CRCI core logic (written in Rust) compiles to a dynamic library (`.so`) with C bindings. UniFFI parses the Rust API and generates idiomatic Kotlin code.

- **Sync boundary:** FFI calls are completely synchronous. Kotlin coroutines can wrap these safely using `Dispatchers.IO` to offload work.
- **Errors:** Rust `Result`s are mapped to Kotlin exceptions.
- **Structs:** CRCI data structures are passed by value and mirrored natively.
