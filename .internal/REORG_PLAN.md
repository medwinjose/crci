# Repository Reorganization Plan (Item 12)

## 1. Target Structure

```text
crci/ (repo root)
├── Cargo.toml                  (workspace root only)
├── .gitignore
├── .github/
├── README.md
├── CHANGELOG.md
├── PROJECT_RULES.md
├── CURRENT_STATE.md
├── REORG_PROGRESS.md
├── REORG_PLAN.md
├── crates/                     (all Rust code)
│   ├── crci-core/              (core library crate)
│   └── crci-node/              (node binary crate)
├── web/
│   └── dashboard/              (frontend code)
├── docs/                       (documentation only)
│   ├── book/
│   ├── spec/
│   ├── tla/
│   └── evidence/               (CI logs & validation proof)
├── scripts/                    (build & tooling scripts)
└── crci-android/               (Android bindings & app)
```

## 2. Directory and File Moves (Old Path -> New Path)

### Crates
- `crci-core/` -> `crates/crci-core/`
- `src/` -> `crates/crci-node/src/` (excluding `src/rr.py` which moves to scripts)
- `tests/` -> `crates/crci-node/tests/`
- `benches/` -> `crates/crci-node/benches/`
- `build.rs` -> `crates/crci-node/build.rs`
- Root `Cargo.toml` (package config) -> `crates/crci-node/Cargo.toml` (Root retains workspace config)

### Web Dashboard
- `docs/dashboard/` -> `web/dashboard/`

### Scripts
- `append_tests.py` -> `scripts/append_tests.py`
- `modify_api.py` -> `scripts/modify_api.py`
- `update.py` -> `scripts/update.py`
- `run_local_proof.ps1` -> `scripts/run_local_proof.ps1`
- `run_proof.ps1` -> `scripts/run_proof.ps1`
- `src/rr.py` -> `scripts/rr.py`

### Evidence/Logs
- `proof_run.log` -> `docs/evidence/proof_run.log`
- `real_wan_log.txt` -> `docs/evidence/real_wan_log.txt`
- `ubuntu_log.txt` -> `docs/evidence/ubuntu_log.txt`
- `chaos_verification_session168.txt` -> `docs/evidence/chaos_verification_session168.txt`

### Deletions / Untracking
- `logs/` -> `git rm -r logs` (disposable scratch)
- `docs/tla/tlc_output.txt` -> untrack and gitignore

## 3. Reference Updates (Grep Hits)

- **`crci-core`**:
  - `Cargo.toml` (`crci-core = { path = "crci-core" }` -> `path = "../crci-core"`)
  - `.github/workflows/ci.yml` (`cd crci-core` -> `cd crates/crci-core`)
  - `.github/workflows/ci.yml` (various uniffi paths)
  - `scripts/cross_build.sh` (needs `crates/crci-core`)
- **`crci` (package)**:
  - Rename to `crci-node` in its own Cargo.toml.
  - Updates in `.github/workflows/ci.yml` (e.g., `cargo build --bin crci` -> `cargo build --bin crci-node -p crci-node`).
- **`docs/dashboard`**:
  - `README.md`, `CURRENT_STATE.md`, `PROJECT_RULES.md`
- **`run_proof.ps1` / Scripts**:
  - `README.md`, `CURRENT_STATE.md`
- **Android / FFI**:
  - Ensure `crci-android` refers to `crates/crci-core` correctly if paths are hardcoded (check `.github/workflows/ci.yml` which explicitly binds UniFFI output).

## 4. Execution Plan
- Run `git mv` and `mkdir` operations.
- Update `Cargo.toml` files and resolve workspace.
- Update `.github/workflows/ci.yml`.
- Update `scripts/` contents to point to `../crates/...`.
- Fix any `cargo check` and `cargo test --workspace` failures.
- Update markdown docs.
