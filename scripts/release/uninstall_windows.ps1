param(
    [string]$Prefix = "$HOME\.local\openjax",
    [switch]$KeepUserData,
    [switch]$Yes
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path $Prefix)) {
    Write-Host "[uninstall] nothing to remove at $Prefix"
    exit 0
}

if (-not $Yes) {
    if ($KeepUserData) {
        $reply = Read-Host "[uninstall] remove OpenJax under $Prefix but keep userdata? [y/N]"
    } else {
        $reply = Read-Host "[uninstall] remove ALL OpenJax files under $Prefix? [y/N]"
    }
    if ($reply -notmatch "^[Yy]$") {
        Write-Host "[uninstall] aborted"
        exit 0
    }
}

$userdata = Join-Path $Prefix "userdata"
if ($KeepUserData -and (Test-Path $userdata)) {
    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("openjax-uninstall-" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $tmpDir | Out-Null
    Move-Item $userdata (Join-Path $tmpDir "userdata")
    Remove-Item -Recurse -Force $Prefix
    New-Item -ItemType Directory -Path $Prefix | Out-Null
    Move-Item (Join-Path $tmpDir "userdata") $userdata
    Remove-Item -Recurse -Force $tmpDir
    Write-Host "[uninstall] removed binaries/configs and kept $userdata"
} else {
    Remove-Item -Recurse -Force $Prefix
    if ($KeepUserData) {
        Write-Host "[uninstall] removed $Prefix (no userdata to keep)"
    } else {
        Write-Host "[uninstall] removed $Prefix"
    }
}
