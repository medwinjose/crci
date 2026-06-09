# Container Deployment

CRCI includes a pre-configured `docker-compose.yml` for multi-node local testing and simulation over a bridge network. This allows verification of TCP handshakes, peer discovery, and node isolation across container boundaries.

## Running the Mesh

To build and spin up the three-node mesh (alpha, beta, and byzantine):

```bash
docker compose up --build
```

### Network Topology

The mesh is deployed on a custom bridge network named `crci-mesh`.

1. **node-alpha** (`0.0.0.0:7001`): The root honest node.
2. **node-beta** (`0.0.0.0:7002`): An honest peer that dials into `node-alpha` on startup.
3. **node-byzantine**: Uses the `byzantine_agent` binary to flood malicious payloads into `node-alpha`, verifying network-level isolation and structural parsing limits.

### Current Limitations

While the internal `crci-core` library supports full asynchronous message parsing, Byzantine severity consensus, and reputation penalties, the user-facing CLI binary (`crci-node`) currently acts as a passive TCP receiver. 

The interactive `send` subcommand for manual message origination from the terminal is planned for a future release. For automated eviction proofs, use the native test harness:
`cargo test --test byzantine_bench`
