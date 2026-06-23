$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$sourceExe = Join-Path $repoRoot "src-tauri\target\release\kill-port.exe"
$releaseDir = Join-Path $repoRoot "release"
$portableExeName = "port-killer.exe"
$targetExe = Join-Path $releaseDir $portableExeName
$hashFile = Join-Path $releaseDir "port-killer.sha256.txt"

function Get-Sha256Hash {
    param([string]$Path)

    if (Get-Command Get-FileHash -ErrorAction SilentlyContinue) {
        return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash
    }

    $certutilOutput = certutil -hashfile $Path SHA256
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to calculate SHA256."
    }

    foreach ($line in $certutilOutput) {
        $trimmed = $line.Trim()
        if ($trimmed -match "^[0-9a-fA-F]{64}$") {
            return $trimmed.ToUpperInvariant()
        }
    }

    throw "Failed to parse SHA256."
}

if (-not (Test-Path -LiteralPath $sourceExe)) {
    throw "Release exe not found. Run pnpm tauri build first."
}

# The release directory contains only the latest portable files.
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null
Get-ChildItem -LiteralPath $releaseDir -File | Remove-Item -Force
Copy-Item -LiteralPath $sourceExe -Destination $targetExe -Force

$hash = Get-Sha256Hash -Path $targetExe
Set-Content -LiteralPath $hashFile -Value "$hash  $portableExeName" -Encoding UTF8

Write-Host "Portable exe: $targetExe"
Write-Host "SHA256: $hash"
