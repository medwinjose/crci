# scripts/

Helper scripts. None of them produce numbers used in the paper; paper numbers come from
`cargo bench` (see [docs/benchmarks.md](../docs/benchmarks.md)).

| Script | Platform | What it does |
|---|---|---|
| `check-docs.sh` | Linux (CI) | Fails if tracked files reintroduce claims known to be false (wrong transport or storage stack, wrong hash, retired BFT IDs, …). Runs in CI. |
| `cross_build.sh` | Linux | Cross-compiles `crci-node` for `armv7` / `aarch64` (see [docs/cross-compilation.md](../docs/cross-compilation.md)). |
| `demo.sh` / `demo.ps1` | Linux / Windows | Starts `node-alpha`, `node-beta` and the web dashboard (Docker Compose, or native with `--native`), sends two messages and starts the Byzantine agent. |
| `test_demo.sh` / `test_demo.ps1` | Linux / Windows | Checks that `docker-compose.yml` parses and the demo scripts exist. |
| `chaos.sh` | Linux + Docker | Runs `cargo test --lib chaos`, then disconnects/kills containers of the Compose mesh and checks the logs for panics. Manual tool; no recorded results. |
