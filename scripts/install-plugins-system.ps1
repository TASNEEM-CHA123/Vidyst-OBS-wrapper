# Elevated helper: moves the Vidyst-OBS plugin folders from the per-user
# staging directory into OBS's supported system-wide plugin directory.
#
# Run through the UAC prompt created by:
#   Start-Process powershell -Verb RunAs -ArgumentList '-File', '<this file>'

$ErrorActionPreference = 'Stop'

$source = Join-Path $env:APPDATA 'obs-studio\plugins'
$destination = 'C:\ProgramData\obs-studio\plugins'
$log = Join-Path $env:TEMP 'vidyst-obs-system-plugin-install.log'

function Write-Log([string]$Message) {
    $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $Message"
    Add-Content -Path $log -Value $line -Encoding utf8
    Write-Host $line
}

try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This helper must run elevated.'
    }

    if (Get-Process -Name obs64 -ErrorAction SilentlyContinue) {
        throw 'OBS is still running. Close OBS before moving plugins.'
    }

    $plugins = @(
        'move-transition',
        'obs-advanced-masks',
        'vertical-canvas',
        'obs-backgroundremoval',
        'obs-multi-rtmp'
    )

    New-Item -ItemType Directory -Force $destination | Out-Null
    Write-Log "Source: $source"
    Write-Log "Destination: $destination"

    foreach ($plugin in $plugins) {
        $from = Join-Path $source $plugin
        $to = Join-Path $destination $plugin

        if (-not (Test-Path $from)) {
            if (Test-Path $to) {
                Write-Log "SKIP $plugin (already in ProgramData)"
                continue
            }
            throw "Missing staged plugin: $from"
        }

        if (Test-Path $to) {
            Remove-Item $to -Recurse -Force
            Write-Log "Removed existing $to"
        }

        Move-Item $from $to
        Write-Log "Moved $plugin -> $to"
    }

    # Remove the now-empty staging folder so it cannot be mistaken for a
    # supported plugin location.
    if ((Test-Path $source) -and -not (Get-ChildItem $source -Force)) {
        Remove-Item $source -Force
        Write-Log 'Removed empty AppData staging folder.'
    }

    Write-Log 'SUCCESS'
    exit 0
}
catch {
    Write-Log "FAILED: $($_.Exception.Message)"
    exit 1
}
