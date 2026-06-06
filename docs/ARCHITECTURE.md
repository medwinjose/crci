# CRCI Architecture

CRCI (Crisis Response Communication Infrastructure) is organized as a Cargo workspace with the following structure:

```mermaid
graph TD
    subgraph "CRCI Workspace"
        crci_bin[crci Binary Crate]
        crci_core[crci_core Library Crate]
        
        crci_bin -->|Depends on| crci_core
    end
    
    subgraph "crci_core Library (Mesh Logic & API)"
        network[Network & Transport Layer]
        gossip[Gossip & Routing Protocol]
        merkle[Merkle DAG State]
        security[Security & Sybil Resistance]
        storage[Encrypted Local Storage]
        api[REST API & WebSockets]
        
        api --> gossip
        gossip --> network
        gossip --> merkle
        merkle --> storage
        network --> security
    end
    
    subgraph "crci Binary (Entrypoint)"
        cli[CLI Argument Parsing]
        main[Main Event Loop & Initialization]
        
        main --> cli
        main --> crci_core
    end
    
    AndroidApp[Future Android App] -.->|Depends on| crci_core
    WebApp[Future Web Interface] -.->|Interacts via| api
```

## Workspace Crates

- **`crci_core`**: The core library crate containing all business logic, protocol rules, cryptography, routing, Sybil resistance, state management, and API interfaces. This is completely decoupled from the binary execution loop, allowing it to be compiled into Android or iOS apps.
- **`crci`**: The primary executable wrapper that handles argument parsing (`clap`), initializing the Tokio asynchronous runtime, setting up signal handlers, and driving the simulation or physical node execution.
