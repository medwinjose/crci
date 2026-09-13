$ErrorActionPreference = 'Stop'

cargo build --release --bin node
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$Jobs = @()

# Honest nodes
for ($i = 1; $i -le 30; $i++) {
    $env:NODE_ID = "honest-node-$i"
    $env:NODE_ZONE = "zone-a"
    $env:NODE_TYPE = "honest"
    $env:PEERS = "127.0.0.1:8080" # They all bind to random ports, wait.
    # Wait, in node.rs, listen_addr is hardcoded to "0.0.0.0:8080"
    # This will cause Address Already In Use if we run them locally!
}
