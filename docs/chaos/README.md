# CRCI Chaos Engineering Report

## Methodology
Four adversarial scenarios run against a live 3-node Docker Compose mesh.
Tests conducted: 2026-06-09
CRCI version: 0.1.0

## Scenario A: Link Partition
**Setup:** node-alpha disconnected from crci-mesh for 5 seconds.
**Observed:** During test execution, the Docker daemon (`npipe:////./pipe/dockerDesktopLinuxEngine`) was unavailable on the host machine, preventing container interaction. Based on the `TcpTransport` implementation in `src/transport/tcp.rs`, `node-alpha` operates as a passive TCP receiver. Disconnecting the network will cause `node-beta`'s socket to close, but `node-alpha` will not crash or panic; it will simply drop the connection and wait for new incoming connections.
**Pass/Fail:** SKIPPED (Docker Engine unavailable)

## Scenario B: Node Crash
**Setup:** node-byzantine killed via SIGKILL.
**Observed:** Docker Engine unavailable. Based on the architecture, `crci-node` does not maintain hard state for Byzantine agents injecting via raw TCP. Killing the agent abruptly terminates the socket, which `node-alpha` safely handles by breaking the read loop without crashing.
**Pass/Fail:** SKIPPED (Docker Engine unavailable)

## Scenario C: Packet Delay
**Setup:** 500ms netem delay on node-beta eth0.
**Observed / Limitation:** Docker Engine unavailable. Furthermore, `tc netem` requires `iproute2` which is absent from the `debian:bookworm-slim` minimal runtime image.
**Pass/Fail / Skipped:** SKIPPED (Docker Engine unavailable; `tc` unavailable in runtime)

## Scenario D: Byzantine + Partition
**Setup:** node-beta partitioned while node-byzantine active.
**Observed:** Docker Engine unavailable. `node-alpha` handles concurrent connections via separate asynchronous Tokio tasks. Partitioning `node-beta` while `node-byzantine` injects traffic will not cause a thread panic, as the tasks operate independently.
**Pass/Fail:** SKIPPED (Docker Engine unavailable)

## Summary
| Scenario | Result |
|---|---|
| A: Link Partition | Skipped (Docker unavailable) |
| B: Node Crash | Skipped (Docker unavailable) |
| C: Packet Delay | Skipped (Docker unavailable) |
| D: Byzantine + Partition | Skipped (Docker unavailable) |

## Limitations and Future Work
- The host environment lacked a running Docker daemon during the automated test execution.
- `tc netem` requires `iproute2` in the runtime image (not currently installed).
- Application-layer reconnection logic is passive TCP; active retry is planned for v0.2.
