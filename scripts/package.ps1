# glaspen2 Windows packaging (pure Rust, no C#)
# Produces a zip payload ready for distribution.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File package.ps1
#   powershell -ExecutionPolicy Bypass -File package.ps1 -SkipRelease

param([switch]$SkipRelease)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

# Kill running instances to avoid file locks
Get-Process -Name "glaspen2" -ErrorAction SilentlyContinue | Stop-Process -Force

$cargoProfile = if ($SkipRelease) { "" } else { "--release" }
$targetDir = if ($SkipRelease) { "target\debug" } else { "target\release" }

$cargoToml = Get-Content Cargo.toml -Raw
$version = if ($cargoToml -match 'version\s*=\s*"([^"]+)"') { $Matches[1] } else { "0.1.0" }
$zipPath = "dist\glaspen2-v$version-windows-x64.zip"

Write-Host "=== glaspen2 v$version - Package Builder ===" -ForegroundColor Cyan

# Step 1: Build Rust launcher
Write-Host "[1/2] Building Rust launcher (glaspen2.exe)..." -ForegroundColor Yellow
& cargo build $cargoProfile.Split(' ')
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
Write-Host "  OK" -ForegroundColor Green

# Step 2: Assemble payload zip
Write-Host "[2/2] Creating zip..." -ForegroundColor Yellow

# Prepare payload directory
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

# VC++ runtime DLLs - required by Rust binary on fresh machines
$vcDlls = @("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll")
foreach ($dll in $vcDlls) {
    $src = "C:\Windows\System32\$dll"
    if (Test-Path $src) {
        Copy-Item $src $payload
        Write-Host "  Bundled $dll" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: $dll not found in System32" -ForegroundColor Yellow
    }
}

# onnxruntime runtime dep (DirectML.dll) - needed by OCR on Windows
$dml = Get-ChildItem "$env:LOCALAPPDATA\ort.pyke.io\dfbin" -Recurse -Filter DirectML.dll -ErrorAction SilentlyContinue | Select-Object -First 1
if ($dml) { Copy-Item $dml.FullName $payload; Write-Host "  Bundled DirectML.dll (OCR)" -ForegroundColor Green }

# Cairo DLLs - required for anti-aliased stroke rendering.
# Prefer bundled vendor/win/cairo, fall back to MSYS2.
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
if (Test-Path $cairoRoot) {
    foreach ($dll in $cairoDlls) {
        $src = Join-Path $cairoRoot $dll
        if (Test-Path $src) {
            Copy-Item $src $payload
            Write-Host "  Bundled $dll" -ForegroundColor Green
        } else {
            Write-Host "  WARNING: $dll not found in $cairoRoot" -ForegroundColor Yellow
        }
    }
    Write-Host "  Cairo DLLs included (from $cairoRoot)" -ForegroundColor Green
} else {
    Write-Host "  WARNING: Cairo DLLs not found (vendor\win\cairo or MSYS2)" -ForegroundColor Yellow
}

# Create ZIP of payload
Remove-Item $zipPath -ErrorAction SilentlyContinue
Compress-Archive -Path "$payload\*" -DestinationPath $zipPath -CompressionLevel Optimal

# Clean up
Remove-Item $payload -Recurse -Force

$size = "{0:N0}" -f (Get-Item $zipPath).Length
Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host "  Package: $zipPath ($size bytes)"
Write-Host ""
Write-Host "  Extract anywhere and run glaspen2.exe"
