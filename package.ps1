# glaspen2 Windows installer builder
# Produces a single self-extracting .exe installer.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File package.ps1
#   powershell -ExecutionPolicy Bypass -File package.ps1 -Debug

param([switch]$Debug)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# Kill running instances to avoid file locks
Get-Process -Name "glaspen2","glaspen2_app" -ErrorAction SilentlyContinue | Stop-Process -Force

$profile = if ($Debug) { "" } else { "--release" }
$targetDir = if ($Debug) { "target\debug" } else { "target\release" }

$cargoToml = Get-Content Cargo.toml -Raw
$version = if ($cargoToml -match 'version\s*=\s*"([^"]+)"') { $Matches[1] } else { "0.1.0" }
$installerExe = "dist\glaspen2-v$version-windows-x64-installer.exe"

Write-Host "=== glaspen2 v$version — Installer Builder ===" -ForegroundColor Cyan

# ── Find C# compiler ──
$cscPaths = @(
    "C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
    "C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe"
)
$csc = $cscPaths | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $csc) { throw "csc.exe not found. .NET Framework 4.x is required." }

# ── Step 1: Build C# overlay ──
Write-Host "[1/4] Building C# overlay (glaspen2_app.exe)..." -ForegroundColor Yellow
Push-Location glaspen2_csharp
try {
    & $csc /target:winexe /unsafe /reference:System.dll /reference:System.Drawing.dll /reference:System.Windows.Forms.dll /reference:Microsoft.CSharp.dll /out:glaspen2_app.exe NativeMethods.cs FakeStrokeForm.cs GlaspenNative.cs HidReader.cs InputWindow.cs OverlayForm.cs PressureForm.cs Program.cs SettingsPipeServer.cs SimpleJson.cs Wintab.cs
    if ($LASTEXITCODE -ne 0) { throw "C# overlay build failed" }
    Write-Host "  OK" -ForegroundColor Green
} finally { Pop-Location }

# ── Step 2: Build C# installer ──
Write-Host "[2/4] Building installer stub..." -ForegroundColor Yellow
Push-Location installer
try {
    & $csc /target:winexe /reference:System.dll /reference:System.IO.Compression.dll /reference:System.IO.Compression.FileSystem.dll /reference:System.Windows.Forms.dll /out:installer.exe Installer.cs
    if ($LASTEXITCODE -ne 0) { throw "Installer build failed" }
    Write-Host "  OK" -ForegroundColor Green
} finally { Pop-Location }

# ── Step 3: Build Rust launcher ──
Write-Host "[3/4] Building Rust launcher (glaspen2.exe)..." -ForegroundColor Yellow
& cargo build $profile.Split(' ')
if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
Write-Host "  OK" -ForegroundColor Green

# ── Step 4: Assemble self-extracting installer ──
Write-Host "[4/4] Creating installer..." -ForegroundColor Yellow

# Prepare payload directory
$payload = "dist\pkg"
Remove-Item -Recurse -Force $payload -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $payload | Out-Null

Copy-Item "$targetDir\glaspen2.exe" $payload
Copy-Item "$targetDir\glaspen2.dll" $payload
Copy-Item "glaspen2_csharp\glaspen2_app.exe" $payload
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

# VC++ runtime DLLs — required by Rust binary on fresh machines
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

# Create ZIP of payload
$zipPath = "dist\payload.zip"
Remove-Item $zipPath -ErrorAction SilentlyContinue
Compress-Archive -Path "$payload\*" -DestinationPath $zipPath -CompressionLevel Optimal

# Combine: installer.exe + payload.zip → self-extracting installer
Remove-Item $installerExe -ErrorAction SilentlyContinue

$installerBytes = [System.IO.File]::ReadAllBytes((Resolve-Path "installer\installer.exe"))
$zipBytes = [System.IO.File]::ReadAllBytes((Resolve-Path $zipPath))

$combined = New-Object byte[] ($installerBytes.Length + $zipBytes.Length)
[Array]::Copy($installerBytes, 0, $combined, 0, $installerBytes.Length)
[Array]::Copy($zipBytes, 0, $combined, $installerBytes.Length, $zipBytes.Length)

[System.IO.File]::WriteAllBytes($installerExe, $combined)

# Clean up
Remove-Item $payload -Recurse -Force
Remove-Item $zipPath -Force

$size = "{0:N0}" -f (Get-Item $installerExe).Length
Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host "  Installer: $installerExe ($size bytes)"
Write-Host ""
Write-Host "  Usage: run $installerExe"
Write-Host "  It will install glaspen2 to %LOCALAPPDATA%\glaspen2"
Write-Host "  and create a Start Menu shortcut."
