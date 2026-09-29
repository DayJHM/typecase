<#
.SYNOPSIS
  Fails the build if the portable exe imports a DLL that Windows does not ship.

.DESCRIPTION
  Typecase's portable build is a single self-contained file: it is copied to a
  USB stick, a Downloads folder or a share and run with nothing beside it. That
  property holds today because the WebView2 binding links WebView2LoaderStatic
  under MSVC (webview2-com-sys), the frontend is embedded in the binary and the
  catalog falls back to an embedded snapshot — so the only machine-level
  dependency is the WebView2 *runtime*, which ships with Windows 10 (April 2018+)
  and Windows 11, i.e. inside our 1809 floor.

  Nothing else in the build would notice if that changed. A new Cargo feature, a
  crate that links its own loader, or a Tauri release that stops linking the
  static loader would put a sibling DLL back in the requirements, and the exe
  would still compile, still package, and still pass every other check — it
  would break only on a user's machine that lacks the DLL. So this step reads
  the exe's import tables and compares them against the Windows DLLs.

  Both tables are read. The delay-import directory matters as much as the
  ordinary one: a delay-loaded DLL is a genuine runtime dependency that does not
  appear in the import directory, which makes it the quiet way to reintroduce
  one.

  The PE headers are walked directly rather than shelling out to dumpbin, so the
  check has no toolchain dependency and runs in the same step as the build. The
  walk was validated against `objdump -p` on the released v0.1.0-rc.2 portable
  exe (identical 25-name set).

  Usage:
    pwsh -NoProfile -ExecutionPolicy Bypass -File packaging/verify-portable-imports.ps1 `
      -Path ../Typecase-portable.exe

  Exits 0 when every imported DLL is on the allow list and the parse found a
  plausible import table; throws (exit 1) otherwise.

  -AllowList replaces the default list. It exists so CI can prove the guard can
  fail (it runs once with the real list, expecting success, and once with
  'kernel32.dll' alone, expecting a rejection): a guard that silently cannot
  fail is worse than no guard at all. It is not a way to make a red build green.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Path,

    [string[]] $AllowList
)

$ErrorActionPreference = 'Stop'

# The Windows DLLs the portable exe is allowed to import: exactly the import set
# of the v0.1.0-rc.2 portable build (sha256 30111020e09776338006be58eba668da95
# 2e86337cee2cec1094d5c7b07722c8), confirmed with objdump -p and by this
# script's own walk. Every entry is a component Windows ships, so none of them
# needs to travel with the exe.
#
# Adding an entry is a deliberate decision, not a fix for a red build: an import
# that is not here means the exe depends on something a bare Windows install may
# not have. See the failure message below.
#
# `api-ms-win-*` are Windows API sets — forwarders resolved by the OS itself
# (kernelbase / the Universal CRT), part of Windows since 10. Their exact
# versioned names are pinned on purpose so a shift is reviewed rather than
# absorbed silently.
$OsDllAllowList = @(
    # Windows system DLLs
    'advapi32.dll'
    'bcrypt.dll'
    'bcryptprimitives.dll'   # ProcessPrng, used by Rust's std
    'combase.dll'
    'comctl32.dll'
    'dwmapi.dll'
    'gdi32.dll'
    'imm32.dll'
    'kernel32.dll'
    'ntdll.dll'
    'ole32.dll'
    'oleaut32.dll'
    'shell32.dll'
    'shlwapi.dll'
    'user32.dll'
    'ws2_32.dll'
    # Windows API sets
    'api-ms-win-core-synch-l1-2-0.dll'
    'api-ms-win-core-winrt-error-l1-1-0.dll'
    'api-ms-win-crt-convert-l1-1-0.dll'
    'api-ms-win-crt-heap-l1-1-0.dll'
    'api-ms-win-crt-locale-l1-1-0.dll'
    'api-ms-win-crt-math-l1-1-0.dll'
    'api-ms-win-crt-runtime-l1-1-0.dll'
    'api-ms-win-crt-stdio-l1-1-0.dll'
    'api-ms-win-crt-string-l1-1-0.dll'
)

if ($PSBoundParameters.ContainsKey('AllowList')) {
    if ($AllowList.Count -eq 0) { throw 'verify-portable-imports: -AllowList was given but is empty' }
    foreach ($entry in $AllowList) {
        if ([string]::IsNullOrWhiteSpace($entry)) { throw 'verify-portable-imports: -AllowList contains an empty entry' }
    }
    $allowed = @($AllowList | ForEach-Object { $_.ToLowerInvariant().Trim() })
    Write-Host "verify-portable-imports: using a caller-supplied allow list of $($allowed.Count) name(s)." -ForegroundColor Yellow
} else {
    $allowed = $OsDllAllowList
}

if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "verify-portable-imports: no such file: $Path"
}
$exe = Get-Item -LiteralPath $Path
$bytes = [System.IO.File]::ReadAllBytes($exe.FullName)
$len = $bytes.Length

# Reads are bounds-checked and the helpers are used for every field, so a
# malformed image produces a clear error instead of a silent wrong answer.
# BitConverter is little-endian on the machine, which every architecture that
# runs Windows is.
function U16([long] $o) {
    if ($o -lt 0 -or ($o + 2) -gt $len) { throw ('read past end of file at 0x{0:X}' -f $o) }
    [BitConverter]::ToUInt16($bytes, [int]$o)
}
function U32([long] $o) {
    if ($o -lt 0 -or ($o + 4) -gt $len) { throw ('read past end of file at 0x{0:X}' -f $o) }
    [BitConverter]::ToUInt32($bytes, [int]$o)
}
function U64([long] $o) {
    if ($o -lt 0 -or ($o + 8) -gt $len) { throw ('read past end of file at 0x{0:X}' -f $o) }
    [BitConverter]::ToUInt64($bytes, [int]$o)
}

if ($len -lt 0x40) { throw "$($exe.Name) is too small to be a PE image ($len bytes)" }
if ($bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw "$($exe.Name) is not a PE image (no MZ header)" }
$peOff = [long](U32 0x3C)
if ($bytes[$peOff] -ne 0x50 -or $bytes[$peOff + 1] -ne 0x45 -or
    $bytes[$peOff + 2] -ne 0x00 -or $bytes[$peOff + 3] -ne 0x00) {
    throw "$($exe.Name) is not a PE image (no PE signature at 0x$($peOff.ToString('X')))"
}
$numSections = [int](U16 ($peOff + 6))
$optSize = [int](U16 ($peOff + 20))
$optOff = $peOff + 24
$magic = [int](U16 $optOff)
switch ($magic) {
    0x20B { $ddOff = $optOff + 112; $imageBase = [long](U64 ($optOff + 24)); $bits = 64 }
    0x10B { $ddOff = $optOff + 96;  $imageBase = [long](U32 ($optOff + 28)); $bits = 32 }
    default { throw ('{0} has an unexpected optional header magic 0x{1:X}' -f $exe.Name, $magic) }
}

# Data directories: index 1 is the import table, index 13 the delay-load table.
$importRva = [long](U32 ($ddOff + 8 * 1))
$delayRva = [long](U32 ($ddOff + 8 * 13))

$sections = @()
for ($i = 0; $i -lt $numSections; $i++) {
    $o = $optOff + $optSize + 40 * $i
    if ($o + 40 -gt $len) { throw "section header $i runs past the end of $($exe.Name)" }
    $nameBytes = $bytes[$o..($o + 7)]
    $sections += [pscustomobject]@{
        Name    = (-join ($nameBytes | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }))
        Virtual = [long](U32 ($o + 12))
        VSize   = [long](U32 ($o + 8))
        RawSize = [long](U32 ($o + 16))
        RawPtr  = [long](U32 ($o + 20))
    }
}

function RvaToOffset([long] $rva, [string] $what) {
    foreach ($s in $script:sections) {
        $end = $s.Virtual + [Math]::Max($s.VSize, $s.RawSize)
        if ($rva -ge $s.Virtual -and $rva -lt $end) {
            if ($rva -ge $s.Virtual + $s.RawSize) {
                throw ('{0} (RVA 0x{1:X}) falls in the virtual-only part of section {2}, so it is not in the file' -f
                       $what, $rva, $s.Name)
            }
            return $rva - $s.Virtual + $s.RawPtr
        }
    }
    throw ('cannot map {0} (RVA 0x{1:X}) to a file offset' -f $what, $rva)
}

function ReadDllName([long] $rva, [string] $what) {
    $o = RvaToOffset $rva $what
    $sb = [System.Text.StringBuilder]::new()
    while ($o -lt $len) {
        $c = $bytes[$o]
        if ($c -eq 0) { break }
        [void]$sb.Append([char]$c)
        $o++
    }
    $name = $sb.ToString()
    if ($name.Length -eq 0) { throw "$what has an empty DLL name" }
    if ($name.Length -gt 260 -or $name -notmatch '^[\x20-\x7E]+$') {
        throw "$what has an implausible DLL name ('$name') — the image or this walk is wrong"
    }
    return $name
}

$found = New-Object System.Collections.Generic.List[string]

if ($importRva -ne 0) {
    $o = RvaToOffset $importRva 'import directory'
    while ($true) {
        if ($o + 20 -gt $len) { throw 'import descriptor runs past the end of the file' }
        if ([long](U32 $o) -eq 0 -and [long](U32 ($o + 12)) -eq 0 -and [long](U32 ($o + 16)) -eq 0) { break }
        $nameRva = [long](U32 ($o + 12))
        if ($nameRva -eq 0) { throw 'import descriptor has no DLL name' }
        $found.Add((ReadDllName $nameRva 'import name'))
        $o += 20
        if ($found.Count -gt 512) { throw 'import table does not terminate — refusing to guess' }
    }
}
$importCount = $found.Count

if ($delayRva -ne 0) {
    $o = RvaToOffset $delayRva 'delay-import directory'
    while ($true) {
        if ($o + 32 -gt $len) { throw 'delay-import descriptor runs past the end of the file' }
        $allZero = $true
        for ($k = 0; $k -lt 8; $k++) { if ([long](U32 ($o + 4 * $k)) -ne 0) { $allZero = $false } }
        if ($allZero) { break }
        $attrs = [long](U32 $o)
        $dllRva = [long](U32 ($o + 4))
        if ($dllRva -eq 0) { throw 'delay-import descriptor has no DLL name' }
        # Bit 0 of the attributes means the descriptor holds virtual addresses
        # rather than RVAs.
        if (($attrs -band 0x1) -ne 0) { $dllRva = $dllRva - $imageBase }
        $found.Add((ReadDllName $dllRva 'delay-load name'))
        $o += 32
        if ($found.Count -gt 512) { throw 'delay-import table does not terminate — refusing to guess' }
    }
}
$delayCount = $found.Count - $importCount

$imported = @($found | ForEach-Object { $_.ToLowerInvariant() } | Sort-Object -Unique)

# Sanity-check the parse before trusting a pass: an empty or absurdly small
# result means something is wrong (wrong architecture, a truncated download, a
# change to the PE layout), and it must never read as "clean".
if ($imported.Count -lt 5) {
    throw ("only {0} imported DLL(s) found in {1} — that cannot be right, so this check will not pass" -f
           $imported.Count, $exe.Name)
}
foreach ($expected in @('kernel32.dll', 'user32.dll')) {
    if ($imported -notcontains $expected) {
        throw "$expected is missing from the imports of $($exe.Name) — the walk is wrong, not the exe"
    }
}

Write-Host "Portable exe : $($exe.FullName)"
Write-Host ("Image        : {0:N0} bytes, PE32{1}, {2} sections, image base 0x{3:X}" -f
            $len, $(if ($bits -eq 64) { '+' } else { '' }), $numSections, $imageBase)
Write-Host ("Import table : RVA 0x{0:X}, {1} DLL(s)" -f $importRva, $importCount)
Write-Host ("Delay imports: {0}" -f
            $(if ($delayRva -eq 0) { 'none (no delay-import directory)' } else { "$delayCount DLL(s)" }))
Write-Host "Imported DLLs ($($imported.Count)), all supplied by Windows:"
foreach ($dll in $imported) { Write-Host "  $dll" }

$unexpected = @($imported | Where-Object { $allowed -notcontains $_ })
$stale = @($allowed | Where-Object { $imported -notcontains $_ })

if ($unexpected.Count -gt 0) {
    $list = ($unexpected | ForEach-Object { "  + $_" }) -join "`n"
    throw @"
The portable exe imports DLL(s) Windows does not ship:
$list

The portable build has to stay a single self-contained file: users run it from
any folder (a USB stick, Downloads, a network share) with nothing beside it, so
an import outside the Windows set means the exe would fail to start on a machine
that lacks that DLL — and the package would not contain it. Nothing else in the
build would catch this, which is why the check exists
(M9_DISTRIBUTION_DESIGN.md §2, checklist §7.6).

If the dependency is intended:
  1. Require the DLL to be supplied by Windows 10 1809+/11, or ship it in the
     portable package — in which case §7.6, M9_DISTRIBUTION_DESIGN.md §2 and
     packaging/portable-README.txt all need to say so, because "one file, no
     install" stops being true.
  2. Add it to OsDllAllowList in packaging/verify-portable-imports.ps1 (with a
     comment naming where it comes from and which platform provides it).
  3. Re-run the Windows sweep on a clean VM: the new DLL must be present there.
Otherwise, link the dependency statically (for WebView2 that is what
webview2-com-sys's WebView2LoaderStatic does) or drop it.
"@
}

if ($stale.Count -gt 0) {
    Write-Host "note: allow-listed but no longer imported (prune deliberately, not automatically):" -ForegroundColor Yellow
    foreach ($dll in $stale) { Write-Host "  - $dll" }
}

Write-Host "OK: the portable exe imports only Windows DLLs — it is still a single self-contained file." -ForegroundColor Green
