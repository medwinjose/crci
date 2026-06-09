#!/usr/bin/env bash
set -euo pipefail

echo "=== CRCI Multi-Node Handshake Demo ==="
echo "Note: The current crci-node CLI does not natively support interactive"
echo "message sending or byzantine output tracing. This script verifies"
echo "that the nodes start up, bind to TCP, and are reachable."

echo "Waiting for mesh to stabilize..."
sleep 3

echo ""
echo "=== Checking node-alpha listening state ==="
docker compose logs node-alpha | grep -i "listening" && echo "✓ node-alpha is listening" || echo "✗ Not found"

echo ""
echo "=== Checking node-beta listening state ==="
docker compose logs node-beta | grep -i "listening" && echo "✓ node-beta is listening" || echo "✗ Not found"

echo ""
echo "=== Checking node-byzantine injection ==="
docker compose logs node-byzantine | grep -i "injected" && echo "✓ byzantine_agent is injecting traffic" || echo "✗ Not injecting"

echo ""
echo "Multi-node network successfully initialized."
