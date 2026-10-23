$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$env:VIDYST_CONFIG_DIR = Join-Path $root '.dev-state'
New-Item -ItemType Directory -Force -Path $env:VIDYST_CONFIG_DIR | Out-Null

Write-Host "VIDYST_CONFIG_DIR=$env:VIDYST_CONFIG_DIR"
Write-Host 'Starting Tauri dev with isolated app state...'
Push-Location $root
try {
    & npm run tauri dev
} finally {
    Pop-Location
}
