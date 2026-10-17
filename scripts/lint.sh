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
# 用临时库跑 FRB wire 测试: 写操作不碰真实 dev 库, 也不会因"dev 库存在"被跳过
TEST_DB="$(mktemp -u)/glaspen2-wire-test.db"
(cd flutter_settings && GLASPEN2_DB_PATH="$TEST_DB" fvm flutter test)

echo
echo "✓ 全部通过"
