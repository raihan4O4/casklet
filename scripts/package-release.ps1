param(
    [Parameter(Mandatory = $true)] [string] $SyftPath,
    [string] $OutputDirectory = "release",
    [switch] $Alpha,
    [switch] $AllowDirty
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $repo
if (-not (Test-Path -LiteralPath $SyftPath -PathType Leaf)) { throw "Syft executable not found: $SyftPath" }
if ($AllowDirty -and -not $Alpha) { throw 'Only local Alpha candidates can include uncommitted source.' }
$dirty = [bool](git status --porcelain)
if ($dirty -and -not $AllowDirty) { throw 'Release packaging requires a clean checkout. Local Alpha validation may explicitly use -AllowDirty.' }
. (Join-Path $PSScriptRoot 'release-source.ps1')
$sourceSnapshot = Get-SourceSnapshot -Repository $repo
$version = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages | Where-Object name -eq 'casklet-cli' | Select-Object -ExpandProperty version
if ($Alpha -and $version -notmatch '-alpha\.') { throw 'Alpha packaging requires an Alpha version.' }
$desktopVersion = (Get-Content apps/desktop/package.json -Raw | ConvertFrom-Json).version
$tauriVersion = (Get-Content apps/desktop/src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
if ($version -ne $desktopVersion -or $version -ne $tauriVersion) { throw 'Rust, frontend, and Tauri versions must match.' }

$revision = (git rev-parse HEAD).Trim()
$cargoLock = (Get-FileHash -LiteralPath 'Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
$pnpmLock = (Get-FileHash -LiteralPath 'pnpm-lock.yaml' -Algorithm SHA256).Hash.ToLowerInvariant()
$output = [IO.Path]::GetFullPath((Join-Path $repo $OutputDirectory))
New-Item -ItemType Directory -Force -Path $output | Out-Null

$cliArgs = @('build', '--locked', '--release', '-p', 'casklet-cli')
if ($Alpha) { $cliArgs += @('--features', 'alpha-cleanup') }
& cargo @cliArgs
if ($LASTEXITCODE -ne 0) { throw 'CLI release build failed.' }
Push-Location 'apps/desktop'
try {
    & corepack pnpm build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend release build failed.' }
    $desktopArgs = @('pnpm', 'tauri', 'build', '--bundles', 'nsis', '--config', 'src-tauri/tauri.release.conf.json')
    if ($Alpha) { $desktopArgs += @('--features', 'alpha-cleanup') }
    & corepack @desktopArgs
    if ($LASTEXITCODE -ne 0) { throw 'Desktop NSIS build failed.' }
} finally { Pop-Location }

$cliZip = Join-Path $output "casklet-cli-$version-windows-x64.zip"
Compress-Archive -LiteralPath @((Join-Path $repo 'target/release/casklet.exe'), (Join-Path $repo 'LICENSE'), (Join-Path $repo 'NOTICE')) -DestinationPath $cliZip -Force
$installers = @(Get-ChildItem -LiteralPath (Join-Path $repo 'target/release/bundle/nsis') -Filter "Casklet_${version}_x64-setup.exe" -File)
if ($installers.Count -ne 1) { throw "Expected one NSIS installer, found $($installers.Count)." }
$installer = Join-Path $output $installers[0].Name
Copy-Item -LiteralPath $installers[0].FullName -Destination $installer -Force

$sbom = Join-Path $output 'casklet.spdx.json'
$env:SYFT_CHECK_FOR_APP_UPDATE = 'false'
$env:SYFT_CACHE_DIR = Join-Path $env:TEMP 'casklet-syft-cache'
& $SyftPath "dir:$repo" --exclude './target/**' --exclude './node_modules/**' --exclude './apps/desktop/node_modules/**' --exclude './release/**' --exclude './.casklet-dev/**' --exclude './.git/**' --exclude './.codex/**' --exclude './.agents/**' -o "spdx-json=$sbom"
if ($LASTEXITCODE -ne 0) { throw 'SPDX SBOM generation failed.' }
$licenseNotice = Join-Path $output 'LICENSE-NOTICES.txt'
$packages = (Get-Content -LiteralPath $sbom -Raw | ConvertFrom-Json).packages
$noticeLines = @(
    "Casklet source revision: $revision",
    'This is dependency license evidence, not a completed legal review.',
    'Casklet project license: Apache-2.0 (LICENSE and NOTICE in this release).',
    'Full package evidence: casklet.spdx.json',
    '',
    "Package`tVersion`tDeclared license"
)
$noticeLines += $packages | Sort-Object name, versionInfo | ForEach-Object { "$($_.name)`t$($_.versionInfo)`t$($_.licenseDeclared)" }
$noticeLines | Set-Content -LiteralPath $licenseNotice

Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination (Join-Path $output 'LICENSE') -Force
Copy-Item -LiteralPath (Join-Path $repo 'NOTICE') -Destination (Join-Path $output 'NOTICE') -Force
$snapshotFile = Join-Path $output 'source-snapshot.json'
$sourceSnapshot | Set-Content -LiteralPath $snapshotFile -Encoding utf8
$notes = Join-Path $repo 'docs/product/alpha-release-notes.md'
Copy-Item -LiteralPath $notes -Destination (Join-Path $output 'release-notes.md') -Force
$files = @($snapshotFile, (Join-Path $output 'release-notes.md'), $cliZip, $installer, $sbom, $licenseNotice, (Join-Path $output 'LICENSE'), (Join-Path $output 'NOTICE'))
@{
    revision = $revision
    version = $version
    channel = $(if ($Alpha) { 'alpha' } else { 'review-only' })
    file_cleanup = [bool]$Alpha
    winget_uninstall = $false
    dirty_source = $dirty
    source_snapshot_sha256 = (Get-FileHash -LiteralPath $snapshotFile -Algorithm SHA256).Hash.ToLowerInvariant()
    cargo_lock_sha256 = $cargoLock
    pnpm_lock_sha256 = $pnpmLock
    target = 'windows-x64'
    installer = 'NSIS currentUser'
    artifacts = $files | ForEach-Object { [IO.Path]::GetFileName($_) }
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'provenance.json')
$files += Join-Path $output 'provenance.json'
$hashLines = foreach ($file in $files) {
    $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($file))"
}
$hashLines | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt')
if ((Get-SourceSnapshot -Repository $repo) -ne $sourceSnapshot) { throw 'Source changed during packaging; rebuild the candidate.' }
Write-Output "Release candidate written to $output"
