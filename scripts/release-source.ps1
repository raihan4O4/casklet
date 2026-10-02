# Hash tracked and non-ignored source, including explicit missing tracked files.
# This records local Alpha candidates honestly without inventing a clean Git revision.
function Get-SourceSnapshot {
    param([Parameter(Mandatory = $true)] [string] $Repository)
    $paths = @(& git -C $Repository -c core.quotepath=false ls-files --cached --others --exclude-standard)
    if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate source files.' }
    $entries = foreach ($relative in ($paths | Sort-Object -Unique)) {
        $path = Join-Path $Repository $relative
        $hash = if (Test-Path -LiteralPath $path -PathType Leaf) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
        [ordered]@{ path = $relative; sha256 = $hash }
    }
    ConvertTo-Json -InputObject @($entries) -Depth 3 -Compress
}
