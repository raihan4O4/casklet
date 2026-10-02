param([string] $OutputDirectory = 'release')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$output = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
$provenance = Get-Content -LiteralPath (Join-Path $output 'provenance.json') -Raw | ConvertFrom-Json
if ($provenance.revision -ne (git -C $repo rev-parse HEAD).Trim()) { throw 'Source revision differs from release provenance.' }
if ($provenance.cargo_lock_sha256 -ne (Get-FileHash -LiteralPath (Join-Path $repo 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Cargo.lock differs from release provenance.' }
if ($provenance.pnpm_lock_sha256 -ne (Get-FileHash -LiteralPath (Join-Path $repo 'pnpm-lock.yaml') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'pnpm-lock.yaml differs from release provenance.' }

. (Join-Path $PSScriptRoot 'release-source.ps1')
$snapshotPath = Join-Path $output 'source-snapshot.json'
if ($provenance.source_snapshot_sha256 -ne (Get-FileHash -LiteralPath $snapshotPath -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Source snapshot checksum differs from provenance.' }
if ((Get-Content -LiteralPath $snapshotPath -Raw).Trim() -ne (Get-SourceSnapshot -Repository $repo)) { throw 'Working source differs from packaged source snapshot.' }
$lines = @(Get-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt'))
if ($lines.Count -lt 9) { throw 'Release checksum list is incomplete.' }
$expected = @($provenance.artifacts) + @('provenance.json')
$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($line in $lines) {
    if ($line -notmatch '^([0-9a-f]{64})  ([^\\/]+)$') { throw "Invalid checksum line: $line" }
    $name = $Matches[2]
    if ($name -in @('.', '..') -or $name.Contains(':') -or -not $seen.Add($name) -or $name -notin $expected) { throw "Unexpected or duplicate checksum entry: $name" }
    $file = Join-Path $output $name
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing artifact: $file" }
    $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Matches[1]) { throw "Checksum mismatch: $file" }
}
if ($seen.Count -ne $expected.Count) { throw 'Checksum list differs from the declared artifact inventory.' }
$sbom = Get-Content -LiteralPath (Join-Path $output 'casklet.spdx.json') -Raw | ConvertFrom-Json
if ($sbom.spdxVersion -ne 'SPDX-2.3' -or -not $sbom.packages) { throw 'SPDX SBOM is missing packages.' }
Write-Output "Verified $($lines.Count) artifacts for $($provenance.revision)."
