#!/bin/bash
# Full build: Rust DLL + exe, C# overlay, DLL copy
set -e

echo "=== Building Rust (DLL + exe) ==="
cargo build "$@"

echo ""
echo "=== Copying glaspen2.dll to C# directory ==="
cp target/debug/glaspen2.dll glaspen2_csharp/glaspen2.dll

echo ""
echo "=== Done ==="
echo "  Rust exe:  target/debug/glaspen2.exe"
echo "  Rust DLL:  target/debug/glaspen2.dll"
echo "  C# overlay: glaspen2_csharp/glaspen2_app.exe"
echo "  DLL (for C#): glaspen2_csharp/glaspen2.dll"
