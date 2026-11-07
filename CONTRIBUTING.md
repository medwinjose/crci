# Contributing to Crisis Response Communication Infrastructure (CRCI)

Thank you for your interest in contributing to CRCI! 

## Getting Started

CRCI is organized as a Cargo workspace. To get started, you'll need the standard Rust toolchain installed.

1. **Clone the repository:**
   ```bash
   git clone https://github.com/medwinjose/crci.git
   cd crci
   ```

2. **Build the entire workspace:**
   ```bash
   cargo build --workspace
   ```

3. **Run the test suite:**
   All pull requests must pass the test suite locally before submission.
   ```bash
   cargo test --workspace
   ```

## Code Quality Standards

We enforce strict formatting and linting rules. Before committing your changes, please run:

1. **Format your code:**
   ```bash
   cargo fmt --all
   ```

2. **Run Clippy (Linter):**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```
   *Note: PRs will fail CI if there are any Clippy warnings.*

## Changing the Protocol

The strike rule, the replay window, the seen cache and anti-entropy are modelled in `docs/tla/CRCI.tla`, and tests
replay the model through the code. A change to any of them changes the model too. Then regenerate the exports and
re-run the checks (needs Java 11+; see `docs/formal-verification.md`):

```bash
python3 docs/tla/check.py --export-transitions docs/tla/reputation-transitions.csv
python3 docs/tla/check.py --export-traces docs/tla/protocol-traces.jsonl
make tla
```

## Pull Request Expectations

- Ensure your code adheres to the existing architectural boundaries (e.g., UI/CLI concerns belong in `crci-node`, not `crci-core`).
- Include tests for new functionality.
- Write descriptive commit messages outlining what was changed and why.
