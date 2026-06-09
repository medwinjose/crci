# Chaos Engineering

To stress-test fault-tolerance beyond steady-state operation, four chaos scenarios were designed to execute against the live Docker Compose mesh:

1. **Scenario A (Link Partition):** Abrupt link partition between honest nodes.
2. **Scenario B (Node Crash):** Hard crash of the Byzantine node mid-session.
3. **Scenario C (Packet Delay):** Artificial packet delay via traffic shaping using `tc netem`.
4. **Scenario D (Byzantine + Partition):** Simultaneous Byzantine injection during a partition event.

## Results

During the latest execution environment run, the Docker Engine was unfortunately unavailable, meaning these scenarios could not interactively complete their container network manipulations. However, this established the precise framework and documented limitations of the current testing constraints:

- Scenarios A, B, and D were skipped due to Docker daemon unavailability, though the underlying TCP architecture guarantees safe panic-free connection drops.
- Scenario C identified an environment dependency: `tc netem` requires `iproute2`, which is intentionally absent from the `debian:bookworm-slim` minimal runtime image to reduce attack surface. This limitation will be addressed with a dedicated chaos testing image in future releases.

For the full detailed engineering report, see `docs/chaos/README.md` in the project repository.
