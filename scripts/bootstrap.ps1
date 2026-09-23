[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$required = @('git', 'rustup', 'cargo', 'node', 'corepack')
$missing = $required | Where-Object { -not (Get-Command $_ -ErrorAction SilentlyContinue) }

if ($missing.Count -gt 0) {
    Write-Error "Missing prerequisites: $($missing -join ', '). See README.md for installation guidance."
}

corepack pnpm --version
cargo xtask bootstrap-check
