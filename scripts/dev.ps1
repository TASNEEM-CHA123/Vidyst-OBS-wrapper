$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$env:VIDYST_CONFIG_DIR = Join-Path $root '.dev-state'
New-Item -ItemType Directory -Force -Path $env:VIDYST_CONFIG_DIR | Out-Null

Write-Host "VIDYST_CONFIG_DIR=$env:VIDYST_CONFIG_DIR"

$portOpen = $false
try {
    $client = [System.Net.Sockets.TcpClient]::new()
    $client.Connect('127.0.0.1', 4455)
    $client.Close()
    $portOpen = $true
} catch {
    $portOpen = $false
}

Push-Location $root
try {
    if ($portOpen) {
        Write-Host 'Ensuring the isolated Vidyst_Dev scene collection is active...'
        & cargo run -q -p vidyst-obs-driver --bin devctl -- setup-dev --state (Join-Path $env:VIDYST_CONFIG_DIR 'original.json')
        if ($LASTEXITCODE -ne 0) { throw "devctl setup-dev failed with exit $LASTEXITCODE" }
    } else {
        Write-Warning 'OBS WebSocket 4455 is not reachable. Start OBS first so the app connects to Vidyst_Dev.'
    }

    Write-Host 'Starting Tauri dev with isolated app state...'
    & npm.cmd run tauri -- dev
} finally {
    Pop-Location
}
