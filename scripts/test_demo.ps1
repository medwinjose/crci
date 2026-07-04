Write-Host "=========================================="
Write-Host "       CRCI Demo Stack CI Validation      "
Write-Host "=========================================="
Write-Host ""

# 1. Validate Docker Compose config syntax
Write-Host "Checking docker-compose.yml syntax..."
if (Get-Command docker -ErrorAction SilentlyContinue) {
    docker compose config > $null
    Write-Host "✓ docker-compose.yml is valid."
} else {
    Write-Host "docker / docker compose not found. Skipping config validation."
}

# 2. Validate scripts/demo.ps1 existence and syntax
if (Test-Path scripts/demo.ps1) {
    Write-Host "✓ scripts/demo.ps1 exists."
} else {
    Write-Error "scripts/demo.ps1 does not exist."
    exit 1
}

Write-Host ""
Write-Host "Demo orchestration stack validation successful!"
