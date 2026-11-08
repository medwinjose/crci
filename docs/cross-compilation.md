# CRCI — Raspberry Pi Deployment Guide

This guide shows how to cross-compile `crci-node` for Raspberry Pi targets directly from an x86-64 machine and run the system as a daemon.

CI cross-compiles both targets on every push (`cross-compile` job in `.github/workflows/ci.yml`). Running on physical Raspberry Pi hardware has not been tested or recorded in this repository.

## Supported Targets
- **ARMv7 (`armv7-unknown-linux-gnueabihf`)**: Raspberry Pi 2, 3, 4 (32-bit OS)
- **AArch64 (`aarch64-unknown-linux-gnu`)**: Raspberry Pi 4, 5 (64-bit OS)

## 1. Prerequisites (Ubuntu/Debian)

To build CRCI for Raspberry Pi locally on an x86-64 Linux machine, you must install the GCC cross-compilers:

```bash
sudo apt-get update
sudo apt-get install -y gcc-arm-linux-gnueabihf gcc-aarch64-linux-gnu
```

## 2. Local Cross-Compilation

We provide a script to handle adding the `rustup` target and building the binary.
Run the script from the root of the repository:

```bash
./scripts/cross_build.sh all
# OR
./scripts/cross_build.sh armv7
./scripts/cross_build.sh aarch64
```

The compiled binaries will be output to `target/<triple>/release/crci-node`.

## 3. Deployment

Transfer the compiled binary to your Raspberry Pi via `scp`.
For example, to transfer to an AArch64 Pi:

```bash
scp target/aarch64-unknown-linux-gnu/release/crci-node pi@raspberrypi.local:~/crci-node
```

## 4. Verification

SSH into your Raspberry Pi and confirm the binary is executable and matches the host architecture:

```bash
ssh pi@raspberrypi.local
file crci-node
./crci-node --version
```

> [!NOTE]
> The encrypted store is not wired to the `crci-node` CLI, so there is no Argon2 memory option
> there. Memory use on 1 GB devices has not been measured.

## 5. Running as a Daemon

To keep CRCI running persistently across reboots, use a `systemd` service.

1. Create a service file:
```bash
sudo nano /etc/systemd/system/crci-node.service
```

2. Add the following configuration (adjusting paths and arguments as needed):
```ini
[Unit]
Description=CRCI Mesh Node
After=network.target

[Service]
Type=simple
User=pi
ExecStart=/home/pi/crci-node --listen 192.168.1.50:7001 --label pi-node --key-file /home/pi/crci-node.key --key-passphrase-file /home/pi/crci-node.pass
Restart=on-failure
RestartSec=5
WorkingDirectory=/home/pi/

[Install]
WantedBy=multi-user.target
```

Use the Pi's own LAN address in `--listen` (here `192.168.1.50`), not `0.0.0.0`: peers answer anti-entropy requests at the listen address, and a node listening on `0.0.0.0` turns anti-entropy off.

`--key-file` keeps the node's key, and so its node ID (derived from the key), across restarts; the file is created on first start and encrypted (AES-256-GCM, key from the passphrase with Argon2id). Put the passphrase on the first line of the passphrase file and make both files readable by the service user only (`chmod 600`). Without `--key-file` the node has a new ID at every start, and its neighbours treat it as a new peer.

3. Enable and start the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable crci-node
sudo systemctl start crci-node
sudo systemctl status crci-node
```
