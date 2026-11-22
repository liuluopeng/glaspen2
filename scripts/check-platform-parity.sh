#!/bin/bash
# 跨平台契约门(纯文本/版本检查, 秒级)。
# 与 cargo test 里的 settings/ui 契约测试互补: 这里拦"面板分支/静默空壳/半实现/版本漂移"。
# macOS CI、Windows CI 与 scripts/lint.sh 都会跑它。
set -e
cd "$(dirname "$0")/.."

fail=0
note() { echo "✗ $*"; fail=1; }

# 1) Flutter 侧不允许平台行为分支(唯一例外: main.bridge.dart 的桥工厂)
branch_hits=$(grep -rn 'Platform\.' flutter_settings/lib --include='*.dart' | grep -v 'lib/src/rust' | grep -v 'lib/main.bridge.dart' || true)
if [ -n "$branch_hits" ]; then
  note "Flutter 出现了平台行为分支(只允许 bridge 工厂):"
  echo "$branch_hits"
fi

# 2) 不允许静默空壳(点一点没反应)
if grep -rn '管道未接入' flutter_settings/lib --include='*.dart' >/dev/null 2>&1; then
  note "存在'管道未接入'式静默空壳; 未实现的功能应由 uiConfig.features 隐藏并抛 UnsupportedError"
fi

# 3) 宁可空缺, 不留半实现: 平台壳里禁止 todo!/unimplemented!
half=$(grep -rnE 'todo!\(|unimplemented!\(' crates/glaspen-windows/src crates/glaspen-core/src --include='*.rs' || true)
if [ -n "$half" ]; then
  note "平台壳/核心存在 todo!/unimplemented!(宁可故意空缺并标记 unsupported):"
  echo "$half"
fi

# 4) 版本单源: Cargo workspace 版本 == Flutter pubspec 版本(去掉 build number)
cargo_v=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
pub_v=$(grep -m1 '^version:' flutter_settings/pubspec.yaml | awk '{print $2}' | cut -d+ -f1)
if [ "$cargo_v" != "$pub_v" ]; then
  note "版本不一致: Cargo.toml=$cargo_v, pubspec.yaml=$pub_v"
fi

# 5) 面板动作表必须由后端下发(不再允许 Dart 里写平台默认键表)
if grep -rn '_shortcutDefaultsWindows\|_shortcutDefaultsMac\|_reservedShortcuts' flutter_settings/lib --include='*.dart' >/dev/null 2>&1; then
  note " Flutter 又出现了本地平台默认键表; 应从 core/ui.rs 经 uiConfig 下发"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "跨平台契约检查失败。规则见 docs/platform-parity.md。"
  exit 1
fi
echo "✓ 跨平台契约检查通过(无平台分支/空壳/半实现, 版本一致)"
