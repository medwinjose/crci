$ErrorActionPreference = 'Stop'

cargo build --bin node --release

Stop-Process -Name node -Force -ErrorAction SilentlyContinue

if (!(Test-Path "logs")) {
    New-Item -ItemType Directory -Path "logs" | Out-Null
}

Remove-Item "logs\*.log" -ErrorAction SilentlyContinue

Write-Host "Spawning 5 nodes..."

# Node 01
$env:NODE_ID="node-01"
$env:NODE_ZONE="zone-a"
$env:LISTEN_PORT="7001"
$env:PEERS="127.0.0.1:7002,127.0.0.1:7003"
$env:IS_BYZANTINE="false"
$env:ROUNDS="12"
Start-Process -FilePath "target\release\node.exe" -RedirectStandardOutput "logs\node-01.log" -RedirectStandardError "logs\node-01.err.log" -NoNewWindow

# Node 02
$env:NODE_ID="node-02"
$env:NODE_ZONE="zone-a"
$env:LISTEN_PORT="7002"
$env:PEERS="127.0.0.1:7001,127.0.0.1:7004"
$env:IS_BYZANTINE="false"
$env:ROUNDS="12"
Start-Process -FilePath "target\release\node.exe" -RedirectStandardOutput "logs\node-02.log" -RedirectStandardError "logs\node-02.err.log" -NoNewWindow

# Node 03
$env:NODE_ID="node-03"
$env:NODE_ZONE="zone-b"
$env:LISTEN_PORT="7003"
$env:PEERS="127.0.0.1:7001,127.0.0.1:7005"
$env:IS_BYZANTINE="false"
$env:ROUNDS="12"
Start-Process -FilePath "target\release\node.exe" -RedirectStandardOutput "logs\node-03.log" -RedirectStandardError "logs\node-03.err.log" -NoNewWindow

# Node 04
$env:NODE_ID="node-04"
$env:NODE_ZONE="zone-b"
$env:LISTEN_PORT="7004"
$env:PEERS="127.0.0.1:7002,127.0.0.1:7005"
$env:IS_BYZANTINE="true"
$env:BYZANTINE_MODE="conflicting"
$env:ROUNDS="12"
Start-Process -FilePath "target\release\node.exe" -RedirectStandardOutput "logs\node-04.log" -RedirectStandardError "logs\node-04.err.log" -NoNewWindow

# Node 05
$env:NODE_ID="node-05"
$env:NODE_ZONE="zone-c"
$env:LISTEN_PORT="7005"
$env:PEERS="127.0.0.1:7003,127.0.0.1:7004"
$env:IS_BYZANTINE="false"
$env:ROUNDS="12"
Start-Process -FilePath "target\release\node.exe" -RedirectStandardOutput "logs\node-05.log" -RedirectStandardError "logs\node-05.err.log" -NoNewWindow

Write-Host "Waiting 20 seconds for processes to finish..."
Start-Sleep -Seconds 20

Write-Host "Gathering proof_run.log..."
$summaries = Get-Content "logs\*.log" | Select-String -Pattern "JSON_SUMMARY:"

if ($summaries.Count -eq 0) {
    Write-Host "ERROR: No JSON summaries found!"
    exit 1
}

# Use Out-File with -Encoding utf8 so it's not utf-16
$summaries | ForEach-Object { $_.Line.Substring("JSON_SUMMARY: ".Length) } | Out-File "proof_run.log" -Encoding utf8

Write-Host "Done! Check proof_run.log"
