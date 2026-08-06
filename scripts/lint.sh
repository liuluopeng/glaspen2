#!/bin/bash
# 发布前的本地质量检查 (fmt + clippy + test)。
# CI 只跑 fmt (macOS CI 无法可靠构建 Flutter framework 做 clippy/test),
# 所以本脚本负责完整检查。用法: scripts/lint.sh
set -e

echo "=== [1/3] rustfmt ==="
cargo fmt --check

echo "=== [2/3] clippy ==="
cargo clippy --all-targets -- -D warnings

echo "=== [3/3] tests ==="
cargo test

echo
echo "✓ 全部通过"
