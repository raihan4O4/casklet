$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Casklet'
$productKey = 'HKCU:\Software\casklet\Casklet'
if ((Test-Path $uninstallKey) -or @(Get-Process casklet-desktop -ErrorAction SilentlyContinue).Count -or (Test-Path (Join-Path $env:LOCALAPPDATA 'Casklet/casklet-desktop.exe'))) { throw 'Existing install/process detected.' }
$oldValues = @()
$hadProductKey = Test-Path $productKey
if ($hadProductKey) {
    $key = Get-Item $productKey
    if ($key.SubKeyCount -ne 0) { throw 'Unexpected existing Casklet subkeys.' }
    $oldValues = @(foreach ($name in $key.GetValueNames()) { @{ Name=$name; Value=$key.GetValue($name); Kind=$key.GetValueKind($name) } })
    $key.Close()
}
$root = Join-Path $repo ('target/alpha-installer-smoke-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $root | Out-Null
$program = [IO.Path]::GetFullPath((Join-Path $root 'program'))
$allowed = [IO.Path]::GetFullPath((Join-Path $repo 'target')) + [IO.Path]::DirectorySeparatorChar
if (-not $program.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)) { throw 'Program path escaped workspace.' }
$installer = Join-Path $repo 'release/alpha/Casklet_0.1.0-alpha.1_x64-setup.exe'
function Test-InstalledDesktop {
    param([string] $Executable)
    # PE Optional Header Subsystem must be Windows GUI (2), not Console (3).
    $bytes = [IO.File]::ReadAllBytes($Executable)
    if ($bytes.Length -lt 64 -or [BitConverter]::ToUInt16($bytes, 0) -ne 0x5a4d) { throw 'Invalid desktop executable.' }
    $pe = [BitConverter]::ToInt32($bytes, 60)
    if ($pe -lt 0 -or ($pe + 94) -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550) { throw 'Invalid PE header.' }
    $subsystem = [BitConverter]::ToUInt16($bytes, $pe + 24 + 68)
    if ($subsystem -ne 2) { throw "Desktop has console subsystem $subsystem; a console-close can terminate the app." }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $desktop = Start-Process -FilePath $Executable -PassThru -WindowStyle Hidden
    try {
        do {
            Start-Sleep -Milliseconds 100
            $desktop.Refresh()
            if ($desktop.HasExited) { throw "Installed desktop exited before showing a window: $($desktop.ExitCode)." }
        } while (($desktop.MainWindowHandle -eq 0 -or $desktop.MainWindowTitle -ne 'Casklet') -and $clock.Elapsed.TotalSeconds -lt 30)
        if ($desktop.MainWindowHandle -eq 0 -or $desktop.MainWindowTitle -ne 'Casklet') { throw 'Installed desktop never revealed its Casklet window.' }
        $descendants = [Collections.Generic.HashSet[int]]::new()
        $null = $descendants.Add($desktop.Id)
        $processes = @(Get-CimInstance Win32_Process)
        do {
            $added = $false
            foreach ($child in $processes) {
                if ($descendants.Contains([int]$child.ParentProcessId) -and $descendants.Add([int]$child.ProcessId)) { $added = $true }
            }
        } while ($added)
        $shells = @($processes | Where-Object { $descendants.Contains([int]$_.ProcessId) -and $_.Name -match '^(cmd|powershell|pwsh|WindowsTerminal|conhost)\.exe$' })
        if ($shells.Count) { throw "Unexpected desktop shell descendants: $($shells.Name -join ', ')." }
        @{ visible_window_ms=$clock.ElapsedMilliseconds; gui_subsystem=$subsystem; unexpected_shell_descendants=$shells.Count; title=$desktop.MainWindowTitle }
    } finally {
        # Only terminate the exact fixture process started above.
        if (-not $desktop.HasExited) { Stop-Process -Id $desktop.Id; $desktop.WaitForExit(10000) | Out-Null }
        $desktop.Dispose()
    }
}
$oldLocal = $env:LOCALAPPDATA
$oldProfile = $env:USERPROFILE
try {
    $installed = Start-Process -FilePath $installer -ArgumentList @('/S','/NS',"/D=$program") -WindowStyle Hidden -PassThru -Wait
    if ($installed.ExitCode -ne 0 -or -not (Test-Path (Join-Path $program 'casklet-desktop.exe'))) { throw 'Silent install failed.' }
    if ((Get-ItemProperty $uninstallKey).DisplayVersion -ne '0.1.0-alpha.1') { throw 'Installed version differs.' }
    $env:LOCALAPPDATA = Join-Path $root 'profile/local'
    $env:USERPROFILE = Join-Path $root 'profile'
    New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA,$env:USERPROFILE | Out-Null
    $cli = Join-Path $repo 'target/release/casklet.exe'
    $seed = & $cli --output json cask create 'Alpha preservation fixture' | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Could not seed release data.' }
    $sentinel = Join-Path $seed.managedStorageRoot 'content/sentinel.txt'
    New-Item -ItemType Directory -Force -Path (Split-Path $sentinel) | Out-Null
    Set-Content -LiteralPath $sentinel -Value 'Managed fixture must survive uninstall'
    $hash = (Get-FileHash -LiteralPath $sentinel).Hash
    $firstLaunch = Test-InstalledDesktop (Join-Path $program 'casklet-desktop.exe')
    $reinstalled = Start-Process -FilePath $installer -ArgumentList @('/S','/NS',"/D=$program") -WindowStyle Hidden -PassThru -Wait
    if ($reinstalled.ExitCode -ne 0) { throw 'Reinstall failed.' }
    $secondLaunch = Test-InstalledDesktop (Join-Path $program 'casklet-desktop.exe')
    $uninstalled = Start-Process -FilePath (Join-Path $program 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($uninstalled.ExitCode -ne 0) { throw 'Uninstall failed.' }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ((Test-Path (Join-Path $program 'casklet-desktop.exe')) -and $clock.Elapsed.TotalSeconds -lt 10) { Start-Sleep -Milliseconds 200 }
    if (Test-Path (Join-Path $program 'casklet-desktop.exe')) { throw 'Program executable remains.' }
    if ((Get-FileHash -LiteralPath $sentinel).Hash -ne $hash) { throw 'Managed fixture changed.' }
    $reopened = & $cli --output json cask get $seed.id | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or $reopened.id -ne $seed.id) { throw 'Database was not preserved.' }
    if (Test-Path $uninstallKey) { throw 'Uninstall registration remains.' }
    @{ result='PASS'; first_launch=$firstLaunch; reinstall_launch=$secondLaunch; version='0.1.0-alpha.1'; install_exit=$installed.ExitCode; reinstall_exit=$reinstalled.ExitCode; uninstall_exit=$uninstalled.ExitCode; data_reopened=$true; managed_content_preserved=$true; program_removed=$true; root=$root; scope='Local installed-desktop launch, console-subsystem guard, silent install/reinstall/uninstall; not clean-machine upgrade or Windows Settings' } | ConvertTo-Json | Set-Content (Join-Path $root 'result.json')
    Get-Content (Join-Path $root 'result.json')
} finally {
    $env:LOCALAPPDATA = $oldLocal
    $env:USERPROFILE = $oldProfile
    if (-not (Test-Path $uninstallKey) -and (Test-Path $productKey)) {
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Software\casklet\Casklet", $true)
        foreach ($name in $key.GetValueNames()) { if ($name -notin @($oldValues | ForEach-Object Name)) { $key.DeleteValue($name) } }
        foreach ($old in $oldValues) { $key.SetValue($old.Name,$old.Value,$old.Kind) }
        $key.Close()
        if (-not $hadProductKey) { Remove-Item -LiteralPath $productKey }
    }
}
