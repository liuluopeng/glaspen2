#!/bin/bash
# Full build: Rust DLL + exe (pure Rust, no C#)
set -e

echo "=== Building Rust (DLL + exe) ==="
cargo build "$@"

echo ""
echo "=== Done ==="
echo "  Rust exe:  target/debug/glaspen2.exe"
echo "  Rust DLL:  target/debug/glaspen2.dll"
