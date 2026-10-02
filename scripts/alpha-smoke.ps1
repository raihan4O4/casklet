param([string] $Binary = 'target/release/casklet.exe')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$binaryPath = (Resolve-Path (Join-Path $repo $Binary)).Path
$root = Join-Path $repo ("target/alpha-smoke-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $root | Out-Null
$previousDev = $env:CASKLET_DEV_ROOT
$previousLocal = $env:LOCALAPPDATA
$previousProfile = $env:USERPROFILE
function Invoke-CaskletJson {
    param([string[]] $Arguments)
    $result = & $binaryPath --output json @Arguments
    if ($LASTEXITCODE -ne 0) { throw "CLI failed: $($Arguments[0..1] -join ' ')" }
    $result | ConvertFrom-Json
}
function Assert-Alpha {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}
try {
    # Both debug and release binaries are confined to this unique workspace fixture.
    $env:CASKLET_DEV_ROOT = Join-Path $root 'dev'
    $env:LOCALAPPDATA = Join-Path $root 'profile/local'
    $env:USERPROFILE = Join-Path $root 'profile'
    New-Item -ItemType Directory -Force -Path $env:USERPROFILE, $env:LOCALAPPDATA | Out-Null
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $null = Invoke-CaskletJson @('cask','list')
    $clock.Stop()
    $startupMs = $clock.ElapsedMilliseconds
    $cask = Invoke-CaskletJson @('cask','create','Alpha cleanup fixture')
    $fixtures = Join-Path $root 'fixtures'
    New-Item -ItemType Directory -Path $fixtures | Out-Null
    $ownedFile = Join-Path $fixtures 'owned.txt'
    $referencedFile = Join-Path $fixtures 'reference.txt'
    $ownedDirectory = Join-Path $fixtures 'dedicated-folder'
    Set-Content -LiteralPath $ownedFile -Value 'Alpha-owned fixture'
    Set-Content -LiteralPath $referencedFile -Value 'Reference must survive'
    New-Item -ItemType Directory -Path $ownedDirectory | Out-Null
    Set-Content -LiteralPath (Join-Path $ownedDirectory 'nested.txt') -Value 'Dedicated directory fixture'
    $null = Invoke-CaskletJson @('resource','adopt',$cask.id,$ownedFile,'--confirm-ownership')
    $null = Invoke-CaskletJson @('resource','adopt',$cask.id,$ownedDirectory,'--confirm-ownership')
    $null = Invoke-CaskletJson @('resource','add',$cask.id,$referencedFile,'--mode','reference')
    $permanent = Invoke-CaskletJson @('cleanup','create',$cask.id,'--permanent')
    Assert-Alpha (@($permanent.items | Where-Object action -eq 'PRESERVE').Count -eq 2) 'External data must be preserved by permanent plans.'
    $plan = Invoke-CaskletJson @('cleanup','create',$cask.id)
    Assert-Alpha (@($plan.items | Where-Object action -eq 'TRASH').Count -eq 2) 'Owned file and directory must be eligible for Recycle Bin.'
    Assert-Alpha (Test-Path -LiteralPath $ownedFile) 'Creating a plan changed the file.'
    Assert-Alpha (Test-Path -LiteralPath $ownedDirectory) 'Creating a plan changed the directory.'
    $null = Invoke-CaskletJson @('cleanup','approve',$plan.id,'--hash',$plan.planHash)
    & $binaryPath --no-input cleanup execute $plan.id --hash $plan.planHash 2>$null | Out-Null
    Assert-Alpha ($LASTEXITCODE -ne 0) 'Unconfirmed cleanup should fail.'
    Assert-Alpha (Test-Path -LiteralPath $ownedFile) 'Unconfirmed cleanup changed data.'
    $done = Invoke-CaskletJson @('cleanup','execute',$plan.id,'--hash',$plan.planHash,'--yes')
    Assert-Alpha ($done.state -eq 'COMPLETE') 'Cleanup did not complete; inspect the retained fixture and journal.'
    Assert-Alpha (-not (Test-Path -LiteralPath $ownedFile)) 'Owned file was not recycled.'
    Assert-Alpha (-not (Test-Path -LiteralPath $ownedDirectory)) 'Owned directory was not recycled.'
    Assert-Alpha ((Get-Content -LiteralPath $referencedFile -Raw).Trim() -eq 'Reference must survive') 'Reference data changed.'
    $history = @(Invoke-CaskletJson @('cleanup','list','--cask-id',$cask.id))
    Assert-Alpha ($history.Count -ge 2) 'Durable cleanup history is unavailable.'
    $status = Invoke-CaskletJson @('cask','removal-status',$cask.id)
    Assert-Alpha $status.canRemove 'Cask still has unexpected dependencies.'
    $null = Invoke-CaskletJson @('cask','remove',$cask.id,'--yes')
    $backup = Invoke-CaskletJson @('diagnostics','backup')
    Assert-Alpha (Test-Path -LiteralPath $backup.path) 'Metadata backup is missing.'
    [ordered]@{ result = 'PASS'; version = (& $binaryPath --version); os = [Environment]::OSVersion.VersionString; startup_cli_ms = $startupMs; root = $root; real_recycle_bin = $true; reference_preserved = $true; unconfirmed_execution_blocked = $true; external_permanent_preserved = $true; cask_removed = $true; backup_verified = $true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'result.json')
    Write-Output "Alpha smoke passed. Evidence: $root/result.json"
} finally {
    $env:CASKLET_DEV_ROOT = $previousDev
    $env:LOCALAPPDATA = $previousLocal
    $env:USERPROFILE = $previousProfile
}
