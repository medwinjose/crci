# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| 2.1.x | Yes |
| < 2.1 | No |

CRCI is a research prototype. It has not had an external security review and must not be relied on to
protect real communications.

## Reporting a vulnerability

Use a [GitHub private security advisory](https://github.com/medwinjose/crci/security/advisories/new).
Please do not open public issues for security bugs.

## Known limitations

The full list, with the code location of each defence and gap, is in
[docs/threat-model.md](docs/threat-model.md). The most important ones:

- TCP connections are neither authenticated nor encrypted, and the sender identity is self-declared.
- The networked `crci-node` binary does not yet apply signature checks, replay protection or reputation
  bans; these run only in `NodeRuntime` (simulator, tests, benchmarks).
- Messages of type `mce` are accepted without a signature on the signed path.
- Sybil resistance is minimal: new key pairs are free.
- Keys are held in process memory; there is no hardware-backed key storage.
- BLE and LoRa transports are stubs.
