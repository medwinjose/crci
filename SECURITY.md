# Security Policy for Crisis Response Communication Infrastructure (CRCI)

## Supported Versions
| Version | Supported          |
| ------- | ------------------ |
| 2.0.x   | :white_check_mark: |
| < 2.0.0 | :x:                |

## Reporting a Vulnerability

Report vulnerabilities via GitHub private security advisory (Settings → Security → Advisories → New draft advisory). Do not open public issues for security bugs.

## Known Limitations

- No formal Sybil resistance yet (planned).
- LoRa transport is a stub (planned).
- Ed25519 keys stored in memory, not in an HSM (planned).

See `docs/fault-model.md` for the full STRIDE analysis.

## Contact
Please report all security vulnerabilities via [GitHub Private Security Advisories](https://github.com/medwinjose/crci/security/advisories).
