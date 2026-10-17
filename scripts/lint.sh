#!/bin/bash
# 发布前的本地质量检查 (fmt + clippy + test + 面板 analyze/test)。
# 用法: scripts/lint.sh
set -e

echo "=== [1/5] rustfmt ==="
cargo fmt --check

echo "=== [2/5] clippy (全部可移植 crate) ==="
cargo clippy --workspace --exclude glaspen2_installer --all-targets -- -D warnings

echo "=== [3/5] tests (全部可移植 crate) ==="
cargo test --workspace --exclude glaspen2_installer

echo "=== [4/5] flutter analyze ==="
(cd flutter_settings && fvm flutter analyze)

echo "=== [5/5] flutter test ==="
(cd flutter_settings && fvm flutter test)

echo
echo "✓ 全部通过"
