@echo off
REM Full build: Rust DLL + exe (pure Rust, no C#)

echo === Building Rust (DLL + exe) ===
cargo build %*
if errorlevel 1 exit /b 1

echo.
echo === Done ===
echo   Rust exe:   target\debug\glaspen2.exe
echo   Rust DLL:   target\debug\glaspen2.dll
