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

- TCP connections are authenticated (Ed25519 challenge-response, signed frames) but not encrypted.
- Node IDs are derived from public keys, so no one can claim an existing node's ID, but new key pairs
  are free and nothing ties a key to a person or device; labels are not authenticated.
- With `--key-file` the node key is stored encrypted (AES-256-GCM, Argon2id); it is only as safe as the
  passphrase file and the device. Without it a node has a new key, and a new ID, at every start.
- Sybil resistance is minimal.
- Keys are held in process memory; there is no hardware-backed key storage.
- BLE and LoRa transports are stubs.
