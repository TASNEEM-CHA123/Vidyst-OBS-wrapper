param(
    [switch]$StartObs,
    [switch]$RestoreOriginal
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$stateDir = Join-Path $root '.dev-state'
$recordDir = Join-Path $root '.dev-recordings'
$stateFile = Join-Path $stateDir 'original.json'
New-Item -ItemType Directory -Force -Path $stateDir, $recordDir | Out-Null

$settings = [ordered]@{
    obs_host              = '127.0.0.1'
    obs_port              = 4455
    record_format         = 'hybrid_mp4'
    record_path           = $recordDir
    stream_server         = $null
    screen_mode           = 'display'
    window_value          = $null
    window_name           = $null
    camera_connected      = $true
    screen_connected      = $true
    camera_device_value   = $null
    camera_device_name    = $null
    screen_device_value   = $null
    screen_device_name    = $null
    background_mode       = 'none'
    background_color      = '#0b0d12'
    background_image      = $null
}
$settings | ConvertTo-Json | Set-Content -Path (Join-Path $stateDir 'settings.json') -Encoding utf8

if ($StartObs) {
    $obs = 'C:\Program Files\obs-studio\bin\64bit\obs64.exe'
    if (-not (Get-Process obs64 -ErrorAction SilentlyContinue)) {
        if (-not (Test-Path $obs)) { throw "OBS not found at $obs" }
        Start-Process -FilePath $obs -WorkingDirectory (Split-Path $obs)
        Write-Host 'Started OBS; waiting for WebSocket 4455...'
        $ready = $false
        for ($i = 0; $i -lt 60; $i++) {
            try {
                $client = [System.Net.Sockets.TcpClient]::new()
                $client.Connect('127.0.0.1', 4455)
                $client.Close()
                $ready = $true
                break
            } catch {
                Start-Sleep -Milliseconds 500
            }
        }
        if (-not $ready) { throw 'OBS WebSocket did not become reachable on 4455' }
    }
}

Push-Location $root
try {
    if ($RestoreOriginal) {
        & cargo run -p vidyst-obs-driver --bin devctl -- restore --state $stateFile
    } else {
        & cargo run -p vidyst-obs-driver --bin devctl -- reset --state $stateFile
    }
    if ($LASTEXITCODE -ne 0) { throw "devctl failed with exit $LASTEXITCODE" }
} finally {
    Pop-Location
}

Write-Host ''
Write-Host 'Reset complete. Dev app settings point at an isolated folder.'
