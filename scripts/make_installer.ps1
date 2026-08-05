# glaspen2 Windows installer builder (pure Rust, no C#)
# Builds: self-extracting setup exe = installer stub + tar.gz payload
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/make_installer.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/make_installer.ps1 -SkipRelease
#
# NOTE: keep comments ASCII-only. Windows PowerShell 5.1 reads scripts as
# ANSI/GBK; non-BOM UTF-8 Chinese comments can corrupt parsing.

param([switch]$SkipRelease)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

# Kill running instances to avoid file locks
Get-Process -Name "glaspen2","glaspen2_settings" -ErrorAction SilentlyContinue | Stop-Process -Force

$cargoProfile = if ($SkipRelease) { "" } else { "--release" }
$targetDir = if ($SkipRelease) { "target\debug" } else { "target\release" }

$cargoToml = Get-Content Cargo.toml -Raw
$version = if ($cargoToml -match 'version\s*=\s*"([^"]+)"') { $Matches[1] } else { "0.1.0" }
$setupExe = "dist\glaspen2-v$version-windows-x64-setup.exe"

Write-Host "=== glaspen2 v$version - Installer Builder ===" -ForegroundColor Cyan

# Step 1: Build Rust launcher
Write-Host "[1/3] Building Rust launcher (glaspen2.exe)..." -ForegroundColor Yellow
& cargo build $cargoProfile.Split(' ')
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
Write-Host "  OK" -ForegroundColor Green

# Step 2: Build installer stub (workspace: output lands in root target/release)
Write-Host "[2/3] Building installer stub..." -ForegroundColor Yellow
Push-Location installer
try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }
} finally { Pop-Location }
Write-Host "  OK" -ForegroundColor Green

# Step 3: Assemble payload
Write-Host "[3/3] Assembling self-extracting installer..." -ForegroundColor Yellow

$payload = "dist\pkg"
Remove-Item -Recurse -Force $payload -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $payload | Out-Null

Copy-Item "$targetDir\glaspen2.exe" $payload
Copy-Item "$targetDir\glaspen2.dll" $payload
Copy-Item "LICENSE" $payload
Copy-Item "README.md" $payload

# Flutter settings UI
$flutterRelease = "flutter_settings\build\windows\x64\runner\Release"
if (Test-Path "$flutterRelease\glaspen2_settings.exe") {
    Copy-Item "$flutterRelease\glaspen2_settings.exe" $payload
    Copy-Item "$flutterRelease\flutter_windows.dll" $payload
    if (Test-Path "$flutterRelease\data") {
        Copy-Item "$flutterRelease\data" -Destination "$payload\data" -Recurse
    }
    Write-Host "  Flutter settings included" -ForegroundColor Green
} else {
    Write-Host "  WARNING: Flutter settings not found at $flutterRelease" -ForegroundColor Yellow
}

# VC++ runtime DLLs
$vcDlls = @("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll")
foreach ($dll in $vcDlls) {
    $src = "C:\Windows\System32\$dll"
    if (Test-Path $src) { Copy-Item $src $payload }
}

# onnxruntime runtime dep (DirectML.dll) - needed by OCR on Windows
$dml = Get-ChildItem "$env:LOCALAPPDATA\ort.pyke.io\dfbin" -Recurse -Filter DirectML.dll -ErrorAction SilentlyContinue | Select-Object -First 1
if ($dml) { Copy-Item $dml.FullName $payload; Write-Host "  Bundled DirectML.dll (OCR)" -ForegroundColor Green }

# Cairo DLLs - prefer bundled vendor/win/cairo
$cairoDlls = @(
    "libcairo-2.dll", "libpixman-1-0.dll", "libpng16-16.dll",
    "zlib1.dll", "libfontconfig-1.dll", "libfreetype-6.dll",
    "libexpat-1.dll", "libglib-2.0-0.dll", "libharfbuzz-0.dll",
    "libiconv-2.dll", "libintl-8.dll", "libpcre2-8-0.dll",
    "libbz2-1.dll", "libbrotlicommon.dll", "libbrotlidec.dll",
    "libffi-8.dll", "libgraphite2.dll",
    "libgcc_s_seh-1.dll", "libwinpthread-1.dll", "libstdc++-6.dll",
    "libdatrie-1.dll", "libfribidi-0.dll"
)
$cairoRoot = if (Test-Path "vendor\win\cairo") { "vendor\win\cairo" } else { "C:\msys64\mingw64\bin" }
foreach ($dll in $cairoDlls) {
    $src = Join-Path $cairoRoot $dll
    if (Test-Path $src) { Copy-Item $src $payload }
}

# ---- Build tar (uncompressed) into memory ----
function Write-TarHeader($stream, $name, $size, $type) {
    $h = New-Object byte[] 512
    $nameBytes = [Text.Encoding]::UTF8.GetBytes($name)
    [Array]::Copy($nameBytes, 0, $h, 0, [Math]::Min($nameBytes.Length, 100))
    # mode 0644 (files) / 0755 (dirs) at offset 100, octal
    $mode = if ($type -eq '5') { "755" } else { "644" }
    [Text.Encoding]::ASCII.GetBytes($mode + [char]0 + [char]0 + [char]0 + [char]0 + [char]0).CopyTo($h, 100)
    # uid/gid 0 at 108/116
    # size octal 12 bytes at 124
    $sizeOct = ([Convert]::ToString($size, 8)).PadLeft(11, '0')
    [Text.Encoding]::ASCII.GetBytes($sizeOct + [char]0).CopyTo($h, 124)
    # mtime at 136 (0)
    # checksum 8 spaces at 148
    [Text.Encoding]::ASCII.GetBytes('        ').CopyTo($h, 148)
    $h[156] = [byte][char]$type
    # magic ustar at 257
    [Text.Encoding]::ASCII.GetBytes("ustar" + [char]0 + "00").CopyTo($h, 257)
    # compute checksum (sum of all bytes with checksum field as spaces)
    $sum = 0
    foreach ($b in $h) { $sum += $b }
    $sumOct = ([Convert]::ToString($sum, 8)).PadLeft(6, '0')
    [Text.Encoding]::ASCII.GetBytes($sumOct + [char]0 + ' ').CopyTo($h, 148)
    $stream.Write($h, 0, 512)
}

$tarStream = New-Object System.IO.MemoryStream
$payloadRoot = (Resolve-Path $payload).Path.TrimEnd('\')

# Directory entries (parents first)
$allDirs = Get-ChildItem $payload -Recurse -Directory
foreach ($d in ($allDirs | Sort-Object { $_.FullName.Length })) {
    $rel = $d.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
    Write-TarHeader $tarStream $rel 0 '5'
}

$files = Get-ChildItem $payload -Recurse -File
foreach ($f in $files) {
    $rel = $f.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
    Write-TarHeader $tarStream $rel $f.Length '0'
    $in = [IO.File]::OpenRead($f.FullName)
    $in.CopyTo($tarStream)
    $in.Close()
    # 512-byte alignment
    $pad = (512 - ($f.Length % 512)) % 512
    if ($pad -gt 0) {
        $zero = New-Object byte[] $pad
        $tarStream.Write($zero, 0, $pad)
    }
    Write-Host "  Packed $rel" -ForegroundColor Green
}
# tar end: two 512 zero blocks
$end = New-Object byte[] 1024
$tarStream.Write($end, 0, 1024)

# ---- GZip compress ----
$tarBytes = $tarStream.ToArray()
$tarStream.Dispose()
$gz = New-Object System.IO.MemoryStream
$gzip = New-Object System.IO.Compression.GZipStream($gz, [System.IO.Compression.CompressionLevel]::Optimal)
$gzip.Write($tarBytes, 0, $tarBytes.Length)
$gzip.Dispose()
$gzBytes = $gz.ToArray()
$gz.Dispose()
Write-Host "  tar=$($tarBytes.Length) gz=$($gzBytes.Length)" -ForegroundColor Cyan

# ---- Concatenate: stub + tar.gz + len(u64 LE) + MAGIC (absolute end) ----
# Installer reads the last 12 bytes as MAGIC and the 8 bytes before it as
# the payload length, so it is immune to magic-looking bytes inside payload.
$distAbs = (Resolve-Path "dist").Path
$stub = [IO.File]::ReadAllBytes((Resolve-Path "target\release\glaspen2_installer.exe"))
$magic = [Text.Encoding]::ASCII.GetBytes('GLASPEN2PKGX')
$setupName = Split-Path $setupExe -Leaf

try {
    $fs = [System.IO.File]::Create([string](Join-Path $distAbs $setupName))
} catch {
    Write-Host "  CREATE EXCEPTION: $_" -ForegroundColor Red
    exit 1
}
$fs.Write($stub, 0, $stub.Length)
$fs.Write($gzBytes, 0, $gzBytes.Length)
$fs.Write([BitConverter]::GetBytes([uint64]$gzBytes.Length), 0, 8)
$fs.Write($magic, 0, $magic.Length)
$fs.Close()

Remove-Item $payload -Recurse -Force

$size = "{0:N0}" -f (Get-Item $setupExe).Length
Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host "  Installer: $setupExe ($size bytes)"
Write-Host ""
Write-Host "  Run it: installs to %LOCALAPPDATA%\glaspen2,"
Write-Host "  creates Start Menu shortcut and launches glaspen2."
