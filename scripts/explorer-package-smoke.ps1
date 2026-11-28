param(
    [string] $InstallerPath = 'target/release/bundle/nsis/Casklet Explorer QA_0.1.0-alpha.1_x64-setup.exe',
    [string] $SessionPath = 'target/explorer-package-session.json',
    [switch] $SkipLaunch,
    [switch] $Complete
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$sessionFile = [IO.Path]::GetFullPath((Join-Path $repo $SessionPath))
$allowed = [IO.Path]::GetFullPath((Join-Path $repo 'target')) + [IO.Path]::DirectorySeparatorChar
if (-not $sessionFile.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Session file must be inside workspace target.' }
$qaUninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Casklet Explorer QA'
$menuKeys = @('HKCU:\Software\Classes\*\shell\Casklet', 'HKCU:\Software\Classes\Directory\shell\Casklet', 'HKCU:\Software\Classes\Casklet.Explorer', 'HKCU:\Software\Casklet\Explorer', 'HKCU:\Software\Classes\*\shellex\ContextMenuHandlers\Casklet', 'HKCU:\Software\Classes\Directory\shellex\ContextMenuHandlers\Casklet', 'HKCU:\Software\Classes\CLSID\{960e5343-e03e-47cd-b1a3-d58632778824}')
$previousLocal = $env:LOCALAPPDATA
$previousProfile = $env:USERPROFILE
$cli = Join-Path $repo 'target/release/casklet.exe'
function Invoke-FixtureCli {
    param([string[]] $Arguments)
    $result = & $cli --output json @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Fixture CLI failed: $($Arguments[0])" }
    $result | ConvertFrom-Json
}
try {
    if ($Complete) {
        $session = Get-Content -LiteralPath $sessionFile -Raw | ConvertFrom-Json
        if ($session.classId) { $menuKeys += "HKCU:\Software\Classes\CLSID\$($session.classId)" }
        $root = [IO.Path]::GetFullPath($session.root)
        if (-not $root.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixture root escaped workspace target.' }
        $env:LOCALAPPDATA = Join-Path $root 'profile/local'
        $env:USERPROFILE = Join-Path $root 'profile'
        $executable = Join-Path $root 'program/casklet-desktop.exe'
        foreach ($process in @(Get-Process casklet-desktop -ErrorAction SilentlyContinue)) {
            if ($process.Path -eq $executable) { Stop-Process -Id $process.Id; $null = $process.WaitForExit(10000) }
        }
        $uninstaller = Join-Path $root 'program/uninstall.exe'
        $uninstall = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
        if ($uninstall.ExitCode -ne 0) { throw 'QA uninstall failed.' }
        $clock = [Diagnostics.Stopwatch]::StartNew()
        while ((Test-Path -LiteralPath $executable) -and $clock.Elapsed.TotalSeconds -lt 15) { Start-Sleep -Milliseconds 100 }
        if (Test-Path -LiteralPath $executable) { throw 'QA program remains after uninstall.' }
        foreach ($key in $menuKeys) { if (Test-Path -LiteralPath $key) { throw "Explorer registration remains: $key" } }
        if (Test-Path -LiteralPath $qaUninstall) { throw 'QA uninstall registration remains.' }
        if ((Get-FileHash -LiteralPath $session.source).Hash -ne $session.sourceHash) { throw 'Source file changed.' }
        $casks = @(Invoke-FixtureCli @('cask', 'list'))
        if ($casks.Count -ne 8) { throw 'Fixture metadata was not preserved.' }
        if ($session.originalExecutable -and (Get-FileHash -LiteralPath $session.originalExecutable).Hash -ne $session.originalHash) { throw 'Existing Casklet executable changed.' }
        $session | Add-Member -NotePropertyName completed -NotePropertyValue $true -Force
        $session | Add-Member -NotePropertyName uninstallPreservedData -NotePropertyValue $true -Force
        $pendingLibraries = @(Get-ChildItem -LiteralPath (Join-Path $root 'program/explorer') -Filter 'casklet-explorer-*.dll' -File -ErrorAction SilentlyContinue)
        $session | Add-Member -NotePropertyName nativeBinaryRemovalPending -NotePropertyValue ([bool]$pendingLibraries.Count) -Force
        $session | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $sessionFile
        Get-Content -LiteralPath $sessionFile
        return
    }
    if (Test-Path -LiteralPath $qaUninstall) { throw 'An existing QA installation must be completed first.' }
    if (@(Get-Process casklet-desktop -ErrorAction SilentlyContinue).Count) { throw 'Existing Casklet process detected; close it before starting the fixture installer.' }
    foreach ($key in $menuKeys) { if (Test-Path -LiteralPath $key) { throw "Preserve existing Explorer integration: $key" } }
    $installer = (Resolve-Path -LiteralPath (Join-Path $repo $InstallerPath)).Path
    if ([IO.Path]::GetFileName($installer) -notlike 'Casklet Explorer QA_*') { throw 'Use the separately named QA package; never replace the existing installation.' }
    $originalExecutable = Join-Path $previousLocal 'Casklet/casklet-desktop.exe'
    $originalHash = if (Test-Path -LiteralPath $originalExecutable) { (Get-FileHash -LiteralPath $originalExecutable).Hash } else { $null }
    $root = Join-Path $repo ('target/explorer-package-' + [guid]::NewGuid())
    $env:LOCALAPPDATA = Join-Path $root 'profile/local'
    $env:USERPROFILE = Join-Path $root 'profile'
    $program = Join-Path $root 'program'
    New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA,$env:USERPROFILE | Out-Null
    $casks = @(foreach ($index in 1..8) { Invoke-FixtureCli @('cask', 'create', "Explorer Cask $index") })
    $source = Join-Path $root '資料 & notes.txt'
    Set-Content -LiteralPath $source -Value 'Explorer fixture; preserve this file.' -Encoding utf8
    $sourceHash = (Get-FileHash -LiteralPath $source).Hash
    $install = Start-Process -FilePath $installer -ArgumentList @('/S','/NS',"/D=$program") -WindowStyle Hidden -PassThru -Wait
    if ($install.ExitCode -ne 0) { throw 'QA installation failed.' }
    $executable = Join-Path $program 'casklet-desktop.exe'
    if (-not (Test-Path -LiteralPath $executable)) { throw 'Installed executable is missing.' }
    # Persist cleanup information before assertions or launching any fixture process.
    @{ root=$root; source=$source; sourceHash=$sourceHash; casks=$casks; originalExecutable=$(if ($originalHash) { $originalExecutable } else { $null }); originalHash=$originalHash; installer=$installer; installerHash=(Get-FileHash -LiteralPath $installer).Hash; installedExecutable=$executable; completed=$false } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $sessionFile
    # An older loaded adapter can remain during upgrade; bind the compiled-in DLL,
    # rather than rejecting an installation directory containing two versions.
    $classId = (Get-Item -LiteralPath 'HKCU:\Software\Casklet\Explorer').GetValue('ClassId')
    if ($classId -notmatch '^\{960e5343-e03e-47cd-[0-9a-f]{4}-[0-9a-f]{12}\}$') { throw 'Versioned COM identity is missing.' }
    $server = Get-Item -LiteralPath "HKCU:\Software\Classes\CLSID\$classId\InprocServer32"
    $currentLibrary = $server.GetValue('')
    $server.Close()
    Copy-Item -LiteralPath $currentLibrary -Destination (Join-Path $program 'explorer/casklet-explorer-old-fixture.dll')
    $repair = Start-Process -FilePath $executable -ArgumentList '--register-explorer' -WindowStyle Hidden -PassThru -Wait
    if ($repair.ExitCode -ne 0) { throw 'Registration failed with an older adapter still present.' }
    $menu = Get-Item -LiteralPath 'HKCU:\Software\Casklet\Explorer'
    $entries = @($menu.GetValue('Snapshot'))
    if ($entries.Count -ne 12) { throw 'Installed menu must contain five Casks and More casks.' }
    if ($menu.GetValue('Executable') -ne $executable) { throw 'Explorer points to the wrong executable.' }
    foreach ($association in @('*','Directory')) {
        $handler = Get-Item -LiteralPath "HKCU:\Software\Classes\$association\shellex\ContextMenuHandlers\Casklet"
        if ($handler.GetValue('') -ne $classId) { throw 'Shell handler is missing.' }
        $handler.Close()
    }
    $menu.Close()
    $null = Invoke-FixtureCli @('cask','update',$casks[0].id,'--name','Renamed & 資料','--revision','0')
    $menu = Get-Item -LiteralPath 'HKCU:\Software\Casklet\Explorer'
    if ($menu.GetValue('Snapshot')[0] -ne 'Renamed && 資料') { throw 'CLI Cask rename did not refresh Explorer.' }
    $menu.Close()
    $null = Invoke-FixtureCli @('cask','archive',$casks[0].id)
    $menu = Get-Item -LiteralPath 'HKCU:\Software\Casklet\Explorer'
    if ($menu.GetValue('Snapshot')[0] -ne 'Explorer Cask 2') { throw 'Archived Cask still appears in Explorer.' }
    $menu.Close()
    $desktop = $null
    if (-not $SkipLaunch) {
    $desktop = Start-Process -FilePath $executable -ArgumentList @('--explorer',$casks[7].id,'--',('"' + $source + '"')) -WindowStyle Hidden -PassThru
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 100
        $desktop.Refresh()
        if ($desktop.HasExited) { throw "Installed desktop exited: $($desktop.ExitCode)" }
    } while ($desktop.MainWindowHandle -eq 0 -and $clock.Elapsed.TotalSeconds -lt 30)
    if ($desktop.MainWindowHandle -eq 0) { throw 'Installed desktop never revealed a window.' }
    $secondary = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru -Wait
    if ($secondary.ExitCode -ne 0) { throw 'Secondary launch failed.' }
    $running = @(Get-Process casklet-desktop -ErrorAction SilentlyContinue | Where-Object Path -eq $executable)
    if ($running.Count -ne 1) { throw 'Secondary launch created a duplicate desktop process.' }
    }
    @{ root=$root; source=$source; sourceHash=$sourceHash; casks=$casks; originalExecutable=$(if ($originalHash) { $originalExecutable } else { $null }); originalHash=$originalHash; installer=$installer; installerHash=(Get-FileHash -LiteralPath $installer).Hash; installedExecutable=$executable; classId=$classId; processId=$(if ($desktop) { $desktop.Id } else { $null }); registered=$true; cliProjection=$true; singleInstance=[bool]$desktop; completed=$false } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $sessionFile
    Get-Content -LiteralPath $sessionFile
} finally {
    $env:LOCALAPPDATA = $previousLocal
    $env:USERPROFILE = $previousProfile
}
