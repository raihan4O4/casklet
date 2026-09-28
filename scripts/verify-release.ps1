param([string] $OutputDirectory = 'release')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$output = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
$provenance = Get-Content -LiteralPath (Join-Path $output 'provenance.json') -Raw | ConvertFrom-Json
if ($provenance.revision -ne (git -C $repo rev-parse HEAD).Trim()) { throw 'Source revision differs from release provenance.' }
if ($provenance.cargo_lock_sha256 -ne (Get-FileHash -LiteralPath (Join-Path $repo 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Cargo.lock differs from release provenance.' }
if ($provenance.pnpm_lock_sha256 -ne (Get-FileHash -LiteralPath (Join-Path $repo 'pnpm-lock.yaml') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'pnpm-lock.yaml differs from release provenance.' }

$lines = @(Get-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt'))
if ($lines.Count -lt 6) { throw 'Release checksum list is incomplete.' }
foreach ($line in $lines) {
    if ($line -notmatch '^([0-9a-f]{64})  ([^\\/]+)$') { throw "Invalid checksum line: $line" }
    $file = Join-Path $output $Matches[2]
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing artifact: $file" }
    $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Matches[1]) { throw "Checksum mismatch: $file" }
}
$sbom = Get-Content -LiteralPath (Join-Path $output 'casklet.spdx.json') -Raw | ConvertFrom-Json
if ($sbom.spdxVersion -ne 'SPDX-2.3' -or -not $sbom.packages) { throw 'SPDX SBOM is missing packages.' }
Write-Output "Verified $($lines.Count) artifacts for $($provenance.revision)."
