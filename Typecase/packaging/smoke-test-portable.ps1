<#
.SYNOPSIS
  Launches the portable exe and fails if its window never comes up.

.DESCRIPTION
  verify-portable-imports.ps1 proves the portable exe needs no DLL beside it. It
  cannot prove the exe starts: a panic, a missing WebView2 runtime or a window
  that never initialises all give a process that dies or stalls, and only running
  it shows that. This is the tier-1 half of WINDOWS_VALIDATION.md 7.1 and 7.8;
  the manual pass still owns what a runner cannot judge (title bar rendering,
  real font installation, reboot persistence).

  The exe is copied into an empty scratch directory and started from there, so a
  passing run also demonstrates "single file, run in place": nothing else is in
  that directory.

  The pass condition is the *configured main window title*, not "a window
  appeared" and not "the process is still alive". That distinction is load
  bearing: the startup-failure path (src-tauri/src/startup.rs) shows a native
  dialog titled "Typecase - startup failed" and blocks until it is dismissed, so
  a looser check would pass on that dialog (its title also contains "Typecase")
  or wait forever. Comparing against the title taken from tauri.conf.json can
  only be satisfied by the real window.

  On failure it names what it observed - exit code, the window title seen, the
  WebView2 runtime version, whether the session was interactive, and recent
  Application event-log entries - because a windows-subsystem binary prints
  nothing to the job console.

  Usage:
    pwsh -NoProfile -ExecutionPolicy Bypass -File packaging/smoke-test-portable.ps1 `
      -Exe <exe> -ExpectedWindowTitle <title> [-TimeoutSeconds 60]

  Exits 0 when the configured window appears, 1 otherwise.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string] $Exe,
    [Parameter(Mandatory = $true)][string] $ExpectedWindowTitle,
    [int] $TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'
$started = Get-Date

if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "smoke test: no such file: $Exe"
}
if ([string]::IsNullOrWhiteSpace($ExpectedWindowTitle)) {
    throw 'smoke test: -ExpectedWindowTitle is required'
}
$exe = (Get-Item -LiteralPath $Exe).FullName

# Microsoft documents the Evergreen WebView2 runtime under this client id, in
# the 32-bit registry view (and per-user for a per-user install). If none of
# those scopes has a version, the runtime is genuinely absent - the single most
# likely reason a launch fails on a fresh machine.
function Get-WebView2Info {
    $keys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )
    $versions = New-Object System.Collections.Generic.List[string]
    foreach ($key in $keys) {
        try {
            $pv = (Get-ItemProperty -LiteralPath $key -Name pv -ErrorAction Stop).pv
            if ($pv) { $versions.Add($pv) }
        } catch {
            # absent in this scope; another scope may still have it
        }
    }
    if ($versions.Count -gt 0) { return ($versions -join ', ') }

    $tree = Get-ChildItem 'C:\Program Files (x86)\Microsoft\EdgeWebView\Application' -Directory -ErrorAction SilentlyContinue
    if ($tree) {
        return ('installed under EdgeWebView\Application: ' + (($tree | ForEach-Object Name) -join ', '))
    }
    return 'NOT FOUND (no EdgeUpdate pv entry, no EdgeWebView\Application directory)'
}

$scratchRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$scratch = Join-Path -Path $scratchRoot -ChildPath ('typecase-smoke-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$runExe = Join-Path -Path $scratch -ChildPath 'Typecase-portable.exe'
Copy-Item -LiteralPath $exe -Destination $runExe

Write-Host 'Smoke test: launching the portable client, alone, from an empty directory'
Write-Host "  exe     : $exe ($((Get-Item -LiteralPath $exe).Length) bytes)"
Write-Host "  scratch : $scratch"
Write-Host "  contains: $((Get-ChildItem -LiteralPath $scratch | ForEach-Object Name) -join ', ')"
Write-Host "  expected: '$ExpectedWindowTitle'"
Write-Host "  WebView2: $(Get-WebView2Info)"
Write-Host "  interactive=$([Environment]::UserInteractive) session=$((Get-Process -Id $PID).SessionId)"

$proc = $null
$title = $null
$verdict = 'no-window'
try {
    $proc = Start-Process -FilePath $runExe -WorkingDirectory $scratch -PassThru
    Write-Host "  started pid $($proc.Id), waiting up to $TimeoutSeconds s for the window"

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 1
        if ($proc.HasExited) { $verdict = 'exited'; break }
        # A process that exits between the check above and this probe throws when
        # its window properties are read, so treat that as the exit it is rather
        # than letting an exception stand in for the diagnosis.
        $handle = 0
        $seen = ''
        try {
            $proc.Refresh()
            $handle = $proc.MainWindowHandle
            $seen = $proc.MainWindowTitle
        } catch {
            $verdict = 'exited'
            break
        }
        if ($handle -ne 0 -and $seen) {
            if ($title -ne $seen) {
                $title = $seen
                Write-Host "  window $([int](((Get-Date) - $started).TotalSeconds))s in: '$title'"
            }
            if ($title -eq $ExpectedWindowTitle) { $verdict = 'ok'; break }
        }
    }
} finally {
    if ($proc -and -not $proc.HasExited) {
        # /T so the WebView2 child processes go down with it
        try { & taskkill.exe /PID $proc.Id /T /F 2>&1 | Out-Null } catch { }
        Start-Sleep -Milliseconds 500
        if (-not $proc.HasExited) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        }
        Write-Host "  stopped pid $($proc.Id)"
    }
    Remove-Item -Recurse -Force -LiteralPath $scratch -ErrorAction SilentlyContinue
}

$elapsed = [int](((Get-Date) - $started).TotalSeconds)

if ($verdict -eq 'ok') {
    Write-Host "OK: the portable client came up on its own - '$title' appeared after $elapsed s with nothing beside the exe." -ForegroundColor Green
    $dataRoot = Join-Path -Path $env:LOCALAPPDATA -ChildPath 'Typecase'
    if (Test-Path -LiteralPath $dataRoot) {
        Write-Host "  data root created: $dataRoot"
    } else {
        Write-Host "  data root not present at $dataRoot (the app may create it lazily)"
    }
    exit 0
}

Write-Host ''
Write-Host 'FAILED: the portable client did not come up.' -ForegroundColor Red
if ($verdict -eq 'exited') {
    Write-Host ('  the process exited by itself after {0} s with code {1} (0x{1:X})' -f $elapsed, $proc.ExitCode)
} else {
    Write-Host "  still running after $TimeoutSeconds s, but no window was ever shown"
}
if ($title) {
    Write-Host "  window title seen: '$title'"
    if ($title -match 'startup failed') {
        Write-Host '  that is the startup-failure dialog: the app could not create its webview, which on'
        Write-Host '  a runner normally means the WebView2 runtime is missing or broken (checklist 7.8).'
    } else {
        Write-Host "  expected exactly: '$ExpectedWindowTitle'"
    }
} else {
    Write-Host "  no window appeared at all (expected '$ExpectedWindowTitle')"
}
Write-Host "  WebView2: $(Get-WebView2Info)"
Write-Host "  interactive=$([Environment]::UserInteractive) session=$((Get-Process -Id $PID).SessionId)"
$dataRoot = Join-Path -Path $env:LOCALAPPDATA -ChildPath 'Typecase'
Write-Host "  data root present: $(Test-Path -LiteralPath $dataRoot)"
Write-Host '  recent Application event-log entries:'
try {
    $errors = Get-WinEvent -LogName Application -MaxEvents 40 -ErrorAction Stop |
        Where-Object { $_.Level -le 3 -and $_.TimeCreated -ge $started }
    if ($errors) {
        foreach ($entry in ($errors | Select-Object -First 5)) {
            $first = ($entry.Message -split "`r?`n")[0]
            Write-Host "    [$($entry.TimeCreated.ToString('HH:mm:ss'))] $($entry.ProviderName): $first"
        }
    } else {
        Write-Host '    none'
    }
} catch {
    Write-Host "    could not read the event log: $($_.Exception.Message)"
}
Write-Host ''
Write-Host 'Tier-1 half of WINDOWS_VALIDATION.md 7.1/7.8; items 7.6-7.10 still need a real VM pass.'
exit 1
