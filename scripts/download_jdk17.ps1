$url = "https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse"
$zipPath = "c:\Users\medwi\crci\jdk17.zip"
$destPath = "c:\Users\medwi\crci\jdk17"

if (Test-Path $destPath) {
    Remove-Item -Recurse -Force -Path $destPath -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force -Path $destPath

Write-Host "Downloading JDK 17 zip using curl..."
curl.exe -L -o $zipPath $url

Write-Host "Extracting JDK 17..."
Expand-Archive -Path $zipPath -DestinationPath $destPath -Force

Write-Host "Cleaning up zip..."
Remove-Item -Path $zipPath -Force

Write-Host "JDK 17 setup complete!"
Get-ChildItem -Path $destPath
