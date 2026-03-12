Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $IsWindows) {
    throw "[package] error: Windows is required for this package target."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$targetDir = Join-Path $repoRoot "target\release"
$distDir = Join-Path $repoRoot "dist"

$tagVersion = ""
try {
    $tagVersion = (git -C $repoRoot describe --tags --exact-match 2>$null).Trim()
} catch {
    $tagVersion = ""
}

if ($tagVersion) {
    $version = $tagVersion -replace "^v", ""
} else {
    $cargoToml = Join-Path $repoRoot "Cargo.toml"
    $versionLine = Select-String -Path $cargoToml -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if (-not $versionLine) {
        throw "[package] error: unable to resolve version."
    }
    $version = $versionLine.Matches[0].Groups[1].Value
}

$bins = @("tui_next.exe", "openjaxd.exe")
foreach ($bin in $bins) {
    $binPath = Join-Path $targetDir $bin
    if (-not (Test-Path $binPath)) {
        throw "[package] error: missing binary $binPath. run: cargo build --release --locked -p tui_next -p openjaxd"
    }
}

$packageName = "openjax-v$version-windows-x86_64"
$stageDir = Join-Path $distDir $packageName
$archivePath = Join-Path $distDir "$packageName.zip"

if (Test-Path $stageDir) { Remove-Item -Recurse -Force $stageDir }
New-Item -ItemType Directory -Force -Path (Join-Path $stageDir "bin") | Out-Null

Copy-Item (Join-Path $targetDir "tui_next.exe") (Join-Path $stageDir "bin\tui_next.exe") -Force
Copy-Item (Join-Path $targetDir "openjaxd.exe") (Join-Path $stageDir "bin\openjaxd.exe") -Force
Copy-Item (Join-Path $scriptDir "install_windows.ps1") (Join-Path $stageDir "install.ps1") -Force
Copy-Item (Join-Path $scriptDir "uninstall_windows.ps1") (Join-Path $stageDir "uninstall.ps1") -Force
Copy-Item (Join-Path $scriptDir "README-install-windows.md") (Join-Path $stageDir "README-install.md") -Force

if (-not (Test-Path $distDir)) { New-Item -ItemType Directory -Force -Path $distDir | Out-Null }
if (Test-Path $archivePath) { Remove-Item -Force $archivePath }

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $archivePath -Force

$hash = (Get-FileHash $archivePath -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path -Leaf $archivePath)" | Out-File (Join-Path $distDir "SHA256SUMS") -Encoding ascii

Write-Host "[package] archive: $archivePath"
Write-Host "[package] checksums: $(Join-Path $distDir 'SHA256SUMS')"
