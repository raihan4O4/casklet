param([string] $InstallerPath = 'target/release/bundle/nsis/Casklet Direct QA_0.1.0-alpha.1_x64-setup.exe')
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$allowed = [IO.Path]::GetFullPath((Join-Path $repo 'target')) + [IO.Path]::DirectorySeparatorChar
$root = [IO.Path]::GetFullPath((Join-Path $allowed ('direct-smoke-' + [guid]::NewGuid())))
if (-not $root.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixture escaped workspace target.' }
$program = Join-Path $root 'program'
$executable = Join-Path $program 'casklet-desktop.exe'
$qaKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Casklet Direct QA'
if (Test-Path -LiteralPath $qaKey) { throw 'A QA installation already exists; preserve it.' }
$productionKey = 'HKCU:\Software\Casklet\Explorer'
$before = if (Test-Path -LiteralPath $productionKey) { (Get-ItemProperty -LiteralPath $productionKey).Snapshot | ConvertTo-Json -Compress } else { '' }
$previousLocal = $env:LOCALAPPDATA
$previousProfile = $env:USERPROFILE
$installed = $false
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class DirectWindowProbe {
  private delegate bool Callback(IntPtr window, IntPtr parameter);
  [DllImport("user32.dll")] private static extern bool EnumWindows(Callback callback, IntPtr parameter);
  [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int capacity);
  [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr window, out Rect rect);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder text, int capacity);
  public static string VisibleWindow(int process) {
    string found = null;
    EnumWindows((window, _) => {
      uint owner; GetWindowThreadProcessId(window, out owner);
      if (owner != process || !IsWindowVisible(window)) return true;
      var text = new StringBuilder(512); GetWindowText(window, text, 512);
      // The official single-instance plugin owns a transparent, non-activating
      // IPC helper. It is not a Casklet application window.
      if (text.ToString() == "app.casklet.directqa-siw") return true;
      Rect rect; GetWindowRect(window, out rect);
      if (rect.Right <= rect.Left || rect.Bottom <= rect.Top) return true;
      var className = new StringBuilder(256); GetClassName(window, className, 256);
      // Tao also owns a transparent, non-activating layered event target.
      if (className.ToString() == "Tao Thread Event Target") return true;
      found = text.ToString() + " [" + className + "] " + (rect.Right-rect.Left) + "x" + (rect.Bottom-rect.Top); return false;
    }, IntPtr.Zero);
    return found;
  }
}
"@
function Assert-Fixture([bool] $condition, [string] $message) { if (-not $condition) { throw $message } }
function Invoke-Fixture([string[]] $arguments) {
    $output = & (Join-Path $repo 'target/release/casklet.exe') --output json @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Fixture CLI failed.' }
    $output | ConvertFrom-Json
}
function Start-Direct([string] $action, [string] $id, [string] $path) {
    Start-Process -FilePath $executable -ArgumentList @($action, $id, '--', ('"' + $path + '"')) -WindowStyle Hidden -PassThru
}
try {
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $env:LOCALAPPDATA = Join-Path $root 'profile/local'
    $env:USERPROFILE = Join-Path $root 'profile'
    New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA, $env:USERPROFILE | Out-Null
    $setup = Start-Process -FilePath (Join-Path $repo $InstallerPath) -ArgumentList @('/S', ('/D=' + $program)) -WindowStyle Hidden -Wait -PassThru
    Assert-Fixture ($setup.ExitCode -eq 0 -and (Test-Path -LiteralPath $executable)) 'QA installation failed.'
    $installed = $true
    $cask = Invoke-Fixture @('cask', 'create', 'Direct background fixture')
    $owned = Join-Path $root '資料 owned folder'
    New-Item -ItemType Directory -Path $owned | Out-Null
    for ($index = 0; $index -lt 2500; $index++) { [IO.File]::WriteAllText((Join-Path $owned ($index.ToString() + '.txt')), 'dedicated fixture') }
    $reference = Join-Path $root 'reference installer.msi'
    [IO.File]::WriteAllText($reference, 'Not an executable installer; must never run')
    $first = Start-Direct '--explorer-owned' $cask.id $owned
    $second = Start-Direct '--explorer-reference' $cask.id $reference
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $reuseObserved = $false
    while (-not $first.HasExited -or -not $second.HasExited) {
        if ($second.HasExited -and -not $first.HasExited) { $reuseObserved = $true }
        foreach ($process in @($first, $second)) {
            if (-not $process.HasExited) { $visible = [DirectWindowProbe]::VisibleWindow($process.Id); Assert-Fixture ($null -eq $visible) ("A direct action unexpectedly displayed a window: " + $visible) }
        }
        Assert-Fixture ($clock.Elapsed.TotalSeconds -lt 60) 'Background operation did not finish automatically.'
        Start-Sleep -Milliseconds 25
    }
    Assert-Fixture ($first.ExitCode -eq 0 -and $second.ExitCode -eq 0) 'A background process failed.'
    $resources = @(Invoke-Fixture @('resource', 'list', $cask.id))
    Assert-Fixture ($resources.Count -eq 2) 'The forwarded operation was lost or duplicated.'
    Assert-Fixture (@($resources | Where-Object management -eq 'MANAGED').Count -eq 1) 'Owned directory classification is incorrect.'
    Assert-Fixture (@($resources | Where-Object management -eq 'REFERENCED').Count -eq 1) 'Installer reference classification is incorrect.'
    Assert-Fixture ((Get-ChildItem -LiteralPath $owned -File).Count -eq 2500) 'Owned source contents changed.'
    Assert-Fixture ([IO.File]::ReadAllText($reference) -eq 'Not an executable installer; must never run') 'Installer reference contents changed.'
    Assert-Fixture $reuseObserved 'Could not establish running-instance forwarding; inspect the fixture.'
    $nativeSource = Join-Path $root 'native selected installer.exe'
    [IO.File]::WriteAllText($nativeSource, 'Not executable. Native Add as Reference must not run this.')
    $libraries = @(Get-ChildItem -LiteralPath (Join-Path $program 'explorer') -Filter '*.dll' -File)
    Assert-Fixture ($libraries.Count -eq 1) 'Expected the versioned packaged native adapter.'
    $harness = Get-ChildItem -LiteralPath (Join-Path $repo 'target/debug/build') -Filter 'explorer-tests.exe' -File -Recurse | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $harness) { throw 'Build the native tests first.' }
    & $harness.FullName --invoke-reference $libraries[0].FullName $cask.id $nativeSource
    Assert-Fixture ($LASTEXITCODE -eq 0) 'Packaged native HDROP/menu invocation failed.'
    $nativeClock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $running = @(Get-Process casklet-desktop -ErrorAction SilentlyContinue | Where-Object Path -eq $executable)
        foreach ($process in $running) { Assert-Fixture ($null -eq [DirectWindowProbe]::VisibleWindow($process.Id)) 'Native menu invocation displayed an unexpected Casklet window.' }
        $nativeResources = @(Invoke-Fixture @('resource', 'list', $cask.id))
        Assert-Fixture ($nativeClock.Elapsed.TotalSeconds -lt 45) 'Native menu invocation did not finish.'
        Start-Sleep -Milliseconds 100
    } while ($nativeResources.Count -ne 3 -or $running.Count -gt 0)
    Assert-Fixture ([IO.File]::ReadAllText($nativeSource) -eq 'Not executable. Native Add as Reference must not run this.') 'Native installer source changed.'
    [ordered]@{ result = 'PASS'; root = $root; hidden_cold_start = $true; running_instance_forwarded = $true; packaged_native_menu_invoked = $true; automatic_exit = $true; owned_directory = $true; installer_reference_not_executed = $true; original_sources_preserved = $true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'result.json')
    Write-Output "Direct Explorer package smoke passed: $root/result.json"
} finally {
    foreach ($process in @(Get-Process casklet-desktop -ErrorAction SilentlyContinue)) {
        if ($process.Path -eq $executable) { Stop-Process -Id $process.Id; $null = $process.WaitForExit(10000) }
    }
    if ($installed) {
        $uninstall = Start-Process -FilePath (Join-Path $program 'uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
        if ($uninstall.ExitCode -ne 0) { throw 'QA uninstall failed; inspect the retained fixture.' }
        Assert-Fixture (-not (Test-Path -LiteralPath $qaKey)) 'QA uninstall registration remains.'
    }
    $env:LOCALAPPDATA = $previousLocal
    $env:USERPROFILE = $previousProfile
    $after = if (Test-Path -LiteralPath $productionKey) { (Get-ItemProperty -LiteralPath $productionKey).Snapshot | ConvertTo-Json -Compress } else { '' }
    Assert-Fixture ($before -eq $after) 'Existing Explorer projection changed during QA.'
}
