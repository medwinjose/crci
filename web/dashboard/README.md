# CRCI Live Dashboard

A standalone React/Vite dashboard for monitoring a CRCI mesh node in real-time.

![CRCI Dashboard Screenshot Placeholder](./screenshot.png)

## Quickstart

Run this one-liner to start the dashboard on port 5173:

```bash
npm install && npm run dev
```

## Configuration

By default, the dashboard connects to a local CRCI node on `http://localhost:8080`.
To connect to a different node or production deployment, copy `.env.example` to `.env` and configure the URL:

```bash
cp .env.example .env
```

`.env`:
```
VITE_NODE_URL=http://your-node-ip:8080
```

The WebSocket URL will automatically be derived from this base URL.

## Panels

1. **Mesh Topology**: Live view of connected peers using `/api/v1/peers`.
2. **Chain Status**: Local node's Merkle chain head index and hash, with divergence tracking using `/chain/head` and `/health`.
3. **Message Throughput**: Real-time events/sec charted with Recharts, powered by the live WebSocket feed.
4. **Byzantine Alerts**: Real-time alerts over `/ws/divergences` for any detected state drifts or bad actors.
5. **Message Feed**: The latest 50 gossip events, colour-coded by severity.
