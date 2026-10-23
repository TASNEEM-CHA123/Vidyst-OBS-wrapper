# Installs the Vidyst-OBS V1 OBS plugins into the current user's OBS plugin
# directory. No administrator rights are required.
#
# Close OBS before running:
#   powershell -ExecutionPolicy Bypass -File .\scripts\install-plugins.ps1
#
# If OBS is still open and you want the script to close it:
#   .\scripts\install-plugins.ps1 -StopObs
#
# Reinstall/upgrade over existing copies:
#   .\scripts\install-plugins.ps1 -Force

[CmdletBinding()]
param(
    [switch]$StopObs,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$pluginRoot = Join-Path $env:APPDATA 'obs-studio\plugins'
$cache = Join-Path $env:LOCALAPPDATA 'vidyst-obs\plugin-cache'
$work = Join-Path $env:TEMP 'vidyst-obs-plugin-install'

function Assert-ObsClosed {
    $process = Get-Process -Name obs64 -ErrorAction SilentlyContinue
    if (-not $process) { return }

    if (-not $StopObs) {
        throw 'OBS is running. Close OBS and run again, or use -StopObs to close it automatically.'
    }

    Write-Host 'Closing OBS...'
    $process | Stop-Process -Force
    Start-Sleep -Seconds 3
}

function Get-CheckedPackage {
    param(
        [string]$Name,
        [string]$Url,
        [string]$Sha256
    )

    $file = Join-Path $cache "$Name.zip"
    if (Test-Path $file) {
        $actual = (Get-FileHash -Path $file -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -eq $Sha256.ToLowerInvariant()) {
            Write-Host "  cached  $Name"
            return $file
        }
        Remove-Item $file -Force
    }

    Write-Host "  download $Name"
    Invoke-WebRequest -Uri $Url -OutFile $file -UseBasicParsing
    $actual = (Get-FileHash -Path $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256.ToLowerInvariant()) {
        Remove-Item $file -Force
        throw "Checksum mismatch for $Name. Expected $Sha256, got $actual."
    }
    return $file
}

function Remove-TargetIfNeeded {
    param([string]$Target)
    if (-not (Test-Path $Target)) { return }
    if (-not $Force) {
        throw "Target already exists: $Target. Re-run with -Force to replace it."
    }
    Remove-Item $Target -Recurse -Force
}

Assert-ObsClosed
New-Item -ItemType Directory -Force $pluginRoot, $cache | Out-Null
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null

# Pinned to the versions verified for OBS 30-32 on 2026-10-22.
$packages = @(
    @{
        Name = 'move-transition'
        Url = 'https://github.com/exeldro/obs-move-transition/releases/download/3.2.1/move-transition-3.2.1-windows-programdata.zip'
        Sha256 = '54bdd712e10f88fc66ea4abfe25503e3080da66a4afe964b924af38ed7079cc8'
        Kind = 'wrapped-folder'
        Folder = 'move-transition'
    },
    @{
        Name = 'advanced-masks'
        Url = 'https://github.com/FiniteSingularity/obs-advanced-masks/releases/download/v1.6.0/obs-advanced-masks-1.6.0-windows-legacy-x64.zip'
        Sha256 = 'b1121cfa6a1372a1f6180512381fa3929977d0adc882edf6ff5fd29a992c1eaf'
        Kind = 'contents'
        Folder = 'obs-advanced-masks'
    },
    @{
        Name = 'vertical-canvas'
        Url = 'https://github.com/Aitum/obs-vertical-canvas/releases/download/1.6.6/vertical-canvas-windows-programdata.zip'
        Sha256 = 'eba2448e68326f4029531ce3743d83d47f3bd3db50daea140dd41ba0a29d85e7'
        Kind = 'wrapped-folder'
        Folder = 'vertical-canvas'
    },
    @{
        Name = 'background-removal'
        Url = 'https://github.com/royshil/obs-backgroundremoval/releases/download/1.4.1/obs-backgroundremoval-1.4.1-windows-x64.zip'
        Sha256 = '7f9c0118c348111bf384c71e23fd056fd26e999bce90132d91db3017c6fa1b94'
        Kind = 'wrapped-folder'
        Folder = 'obs-backgroundremoval'
    },
    @{
        Name = 'multi-rtmp'
        Url = 'https://github.com/sorayuki/obs-multi-rtmp/releases/download/0.7.4.4/obs-multi-rtmp-0.7.4.4-windows-x64.zip'
        Sha256 = '39dffb45dc2a73db0d8777bde4baec2a8d61736577795e2c592ca0e2c0f6952b'
        Kind = 'legacy-layout'
        Folder = 'obs-multi-rtmp'
    }
)

foreach ($package in $packages) {
    Write-Host "Installing $($package.Name)..."

    $target = Join-Path $pluginRoot $package.Folder
    Remove-TargetIfNeeded -Target $target

    $zip = Get-CheckedPackage -Name $package.Name -Url $package.Url -Sha256 $package.Sha256
    $extract = Join-Path $work $package.Name
    Expand-Archive -Path $zip -DestinationPath $extract -Force

    switch ($package.Kind) {
        'wrapped-folder' {
            $source = Join-Path $extract $package.Folder
            if (-not (Test-Path $source)) {
                throw "Expected $source in $($package.Name)."
            }
            Copy-Item $source $pluginRoot -Recurse
        }
        'contents' {
            New-Item -ItemType Directory -Force $target | Out-Null
            Copy-Item (Join-Path $extract '*') $target -Recurse -Force
        }
        'legacy-layout' {
            New-Item -ItemType Directory -Force (Join-Path $target 'bin\64bit'), (Join-Path $target 'data') | Out-Null
            Copy-Item (Join-Path $extract 'obs-plugins\64bit\*') (Join-Path $target 'bin\64bit') -Recurse -Force
            $data = Join-Path $extract 'data\obs-plugins\obs-multi-rtmp'
            if (Test-Path $data) {
                Copy-Item (Join-Path $data '*') (Join-Path $target 'data') -Recurse -Force
            }
        }
        default { throw "Unknown package kind: $($package.Kind)" }
    }

    Write-Host "  installed -> $target"
}

Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue

Write-Host ''
Write-Host 'Installed plugins:' -ForegroundColor Green
Get-ChildItem $pluginRoot -Directory | Select-Object -ExpandProperty Name | ForEach-Object { Write-Host "  - $_" }
Write-Host ''
Write-Host 'Restart OBS, then verify with:' -ForegroundColor Cyan
Write-Host '  cargo run -p vidyst-obs-driver --bin phase0'
