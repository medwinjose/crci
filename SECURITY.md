# Security Policy

## Supported Versions
| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| Older   | :x:                |

## Reporting a Vulnerability

Report vulnerabilities via GitHub private security advisory (Settings → Security → Advisories → New draft advisory). Do not open public issues for security bugs.

## Known Limitations

- No formal Sybil resistance yet (planned Session 47)
- LoRa transport is a stub (planned Session 43)
- Ed25519 keys stored in memory, not in an HSM (planned Session 33+)

See `docs/fault_model.md` for the full STRIDE analysis.

## Contact
medwinjose on GitHub
