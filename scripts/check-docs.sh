#!/usr/bin/env bash
# Fails if tracked files reintroduce claims that were removed because they are false
# (see docs/architecture.md and docs/threat-model.md). Run from the repository root.
set -euo pipefail

# Files allowed to name these technologies because they discuss them as alternatives or history.
ALLOW=':!docs/adr/001-custom-tcp-transport.md :!CHANGELOG.md :!scripts/check-docs.sh'

fail=0
check() {
  local label="$1" pattern="$2"
  # shellcheck disable=SC2086
  if hits=$(git grep -n -I -i -E "$pattern" -- . $ALLOW); then
    echo "✗ $label"
    echo "$hits" | sed 's/^/    /'
    fail=1
  fi
}

check "transport is custom TCP, not libp2p/QUIC/Noise" '(rust-)?libp2p|\bquic\b|noise (protocol|handshake)|gossipsub|kademlia'
check "storage is AES-256-GCM files, not RocksDB"         'rocksdb'
check "no consensus protocol is implemented"              'pbft variant|variant of (practical )?byzantine fault tolerance'
check "hash chain uses SHA-256"                           '\bsha-?3(-256)?\b'
check "test counts are not hard-coded in prose"           '[0-9]+ passed tests|backed by [0-9]+ tests'
check "preprint is on Zenodo"                             'arxiv'
check "the sliding replay window is the default"         'strict-monotonic by default|sliding window is off by default|under `?paper_v2'
check "vector IDs BFT-011..029 are retired"               'BFT-0(1[1-9]|2[0-9])'
check "planning notes stay out of the repository"         '\btasks? [0-9]+\b|session[ -_]?[0-9]+|the prompt says|recruiter'
check "no tool attribution in the repository"             '\bclaude\b|anthropic|antigravity|gemini|chatgpt|copilot'
check "model checking covers small instances, it is no proof" 'formally (proved|proven|verified)|provabl|verified for all'
check "a gossip copy can follow its sync answer"          'untagged repeats are never excused|same ordered connection'
check "any answer that entered the seen cache excuses its gossip copy" 'follows its accepted answer|copy of a message accepted as an answer'

if [ "$fail" -ne 0 ]; then
  echo "Documentation guard failed."
  exit 1
fi
echo "Documentation guard passed."
