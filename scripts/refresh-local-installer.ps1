param([string] $OutputDirectory = 'release/alpha')

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
# Only promote a checksum- and source-verified package, never an arbitrary EXE.
& (Join-Path $PSScriptRoot 'verify-release.ps1') -OutputDirectory $OutputDirectory
$output = Join-Path $repo $OutputDirectory
$provenance = Get-Content (Join-Path $output 'provenance.json') -Raw | ConvertFrom-Json
$installers = @($provenance.artifacts | Where-Object { $_ -match '^Casklet_.+_x64-setup\.exe$' })
if ($installers.Count -ne 1) { throw 'Expected exactly one verified NSIS setup.' }
$source = Join-Path $output $installers[0]
$sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
$release = Join-Path $repo 'release'
$canonical = Join-Path $release $installers[0]
$legacy = Join-Path $release 'Casklet_0.0.0_x64-setup.exe'
# Preserve old local artifacts as evidence rather than deleting them.
foreach ($destination in @($canonical, $legacy)) {
    if ((Test-Path -LiteralPath $destination) -and (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $sourceHash) {
        $oldHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        $archive = Join-Path $release 'obsolete'
        New-Item -ItemType Directory -Force -Path $archive | Out-Null
        Copy-Item -LiteralPath $destination -Destination (Join-Path $archive ($oldHash + '-' + [IO.Path]::GetFileName($destination)))
    }
    Copy-Item -LiteralPath $source -Destination $destination -Force
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $sourceHash) { throw 'Promoted installer checksum differs.' }
}
@{
    canonical_installer = $canonical
    compatibility_path = $legacy
    compatibility_note = 'The historical 0.0.0 filename is an alias only; use the canonical versioned setup for distribution.'
    sha256 = $sourceHash
    source_revision = $provenance.revision
} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $release 'local-installer.json')
Write-Output "Refreshed local setup paths from verified package: $canonical"
