param(
    [string]$Prefix = "$HOME\.local\openjax",
    [switch]$Yes
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $IsWindows) {
    throw "[install] error: this prebuilt package supports only Windows."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$binSrcDir = Join-Path $scriptDir "bin"

if (-not (Test-Path $binSrcDir)) {
    throw "[install] error: missing bin directory beside install.ps1"
}

if (-not $Yes) {
    $reply = Read-Host "[install] install OpenJax binaries to: $Prefix\bin ? [y/N]"
    if ($reply -notmatch "^[Yy]$") {
        Write-Host "[install] aborted"
        exit 0
    }
}

$destBin = Join-Path $Prefix "bin"
New-Item -ItemType Directory -Force -Path $destBin | Out-Null

Copy-Item (Join-Path $binSrcDir "tui_next.exe") (Join-Path $destBin "tui_next.exe") -Force
Copy-Item (Join-Path $binSrcDir "openjaxd.exe") (Join-Path $destBin "openjaxd.exe") -Force

Write-Host "[install] done: $destBin"
Write-Host "[install] add to PATH if needed:"
Write-Host "  `$env:Path = `"$destBin;`$env:Path`""
