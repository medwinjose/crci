# Architecture

CRCI is built as a layered architecture to ensure separation of concerns between application logic, cryptographic verification, and the physical transport medium. The core is embedded in `crci-core`, functioning as a library that can be integrated into CLI binaries, mobile applications via FFI, and web dashboards via Axum.

## Protocol Stack

```text
+-----------------------------------------------------------+
|                      Application                          |
|    (CLI, Android via FFI, Axum REST/WebSocket API)        |
+-----------------------------------------------------------+
|                      NodeRuntime                          |
|    (Peer Management, Handshake, Reputation Engine,        |
|     Gossip Protocol, AEDA Escalation, Message TTL)        |
+-----------------------------------------------------------+
|                 TransportMultiplexer                      |
|    (Routes packets across multiple physical interfaces)   |
+-----------------------------------------------------------+
|      |      TcpTransport      |   (Async tokio::net)      |
|      +------------------------+                           |
|      |      BleTransport      |   (Bluetooth Low Energy)  |
|      +------------------------+                           |
|      |      LoraTransport     |   (Hardware radio links)  |
+-----------------------------------------------------------+
```

## Node Lifecycle

The lifecycle of a CRCI node ensures it integrates securely into the mesh before actively gossiping:

1. **Initialization (`init`)**: A node creates an instance of `NodeRuntime`. It generates its Ed25519 identity, provisions encrypted local storage (AES-256-GCM), and initializes its K-bucket peer table.
2. **Handshake**: The node connects to known peers or bootstrap nodes. During this phase, it exchanges identity tokens, verifies cryptographic signatures, and establishes trust.
3. **Gossip**: Once connected, the node continuously participates in the gossip protocol. It receives messages, validates their TTL, checks replay filters, verifies signatures, and relays them to its K-bucket peers.
4. **Eviction**: If a peer becomes unresponsive, its battery drains, or it sends invalid/malicious data, the `NodeRuntime` gracefully evicts it from the peer table and severs the transport connection to protect network integrity.

## Transport Layer

The physical transmission of bytes is abstracted behind the `Transport` trait, currently located in `crci-core/src/transport/`. The `TransportMultiplexer` dynamically selects the best available medium:

- **`TcpTransport`**: Built on `tokio::net`, this provides real asynchronous networking over standard IP networks. It's the primary transport used in local testing, the Docker proof loop, and stable infrastructure scenarios.
- **`BleTransport`**: A stubbed interface designed to utilize Bluetooth Low Energy for short-range phone-to-phone mesh networking when cellular towers fail.
- **`LoraTransport`**: A stubbed hardware abstraction layer (HAL) meant for long-range, low-bandwidth radio links between designated base stations across geographic zones.
