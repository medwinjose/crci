# Documentation

| Document | What it covers |
|---|---|
| [architecture.md](architecture.md) | Code layout, the message path (in-process and TCP), modules, binaries, build features. Diagram: [architecture/crci-architecture.html](architecture/crci-architecture.html). |
| [threat-model.md](threat-model.md) | Adversary model, defences that exist, known gaps. |
| [benchmarks.md](benchmarks.md) | Every recorded measurement, with harness, command and raw data. |
| [formal-verification.md](formal-verification.md) | The TLA+ model, how to run TLC, and exactly what the model covers. |
| [cross-compilation.md](cross-compilation.md) | Building `crci-node` for Raspberry Pi targets. |
| [openapi.json](openapi.json) | HTTP/WebSocket API of `crci-node`. |
| [adr/](adr/) | Architecture decision records 001–005. |
| [evidence/](evidence/README.md) | Raw outputs of recorded runs, with SHA-256 hashes. |
| [tla/](tla/) | TLA+ specification and the pinned TLC jar. |
| [understanding/](understanding/README.md) | Guided walkthrough of the project from first principles (in preparation). |

The preprint is published on Zenodo: <https://doi.org/10.5281/zenodo.23032042>. Working drafts are
kept outside this repository.
