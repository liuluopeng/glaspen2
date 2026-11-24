#!/bin/bash
# macOS 原生层(Swift)自动化冒烟:菜单/状态栏自检 + 冷启动 + 事件 tap + 优雅退出。
# 用途:S8 前的最小回归;真机笔/屏/多屏验收仍见 docs/s8-acceptance-checklist.md。
# 用法: sh scripts/smoke-macos.sh [release|debug]   (默认 release)
set -euo pipefail

cd "$(dirname "$0")/.."
PROFILE="${1:-}"
[ -n "$PROFILE" ] || PROFILE=release
BIN="target/$PROFILE/glaspen2"
if [ ! -x "$BIN" ]; then
  echo "缺少 $BIN;先 cargo build --$PROFILE" >&2
  exit 2
fi

PROBE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/glaspen2-probe.XXXXXX")"
PROBE_DB="$PROBE_ROOT/settings.db"
PROBE_DB2="$PROBE_ROOT/pages.db"
PROBE_LOG="$PROBE_ROOT/logs"
"$BIN" --swift-menu-probe
"$BIN" --swift-menu-full-probe
"$BIN" --swift-statusbar-probe
GLASPEN2_DB_PATH="$PROBE_DB" GLASPEN2_LOG_DIR="$PROBE_LOG" "$BIN" --swift-settings-probe
GLASPEN2_DB_PATH="$PROBE_DB2" GLASPEN2_LOG_DIR="$PROBE_LOG" "$BIN" --swift-pages-probe

echo "=== 冷启动冒烟 ==="
TMPBASE="$TMPDIR"
[ -n "$TMPBASE" ] || TMPBASE=/tmp
SCRATCH="$(mktemp -d "$TMPBASE/glaspen2-smoke.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
LOG="$SCRATCH/app.log"
GLASPEN2_DB_PATH="$SCRATCH/smoke.db" \
GLASPEN2_LOG_DIR="$SCRATCH/logs" \
GLASPEN2_ALLOW_MULTI=1 \
  "$BIN" >"$LOG" 2>&1 &
PID=$!

READY=0
for _ in $(seq 1 150); do
  if grep -q "window ready" "$LOG" 2>/dev/null; then READY=1; break; fi
  if ! kill -0 "$PID" 2>/dev/null; then break; fi
  sleep 0.2
done
if [ "$READY" -ne 1 ]; then
  echo "FAIL: 启动未就绪" >&2
  tail -40 "$LOG" >&2
  kill -KILL "$PID" 2>/dev/null || true
  exit 1
fi

grep -q "CGEventTap created OK" "$LOG" || echo "WARN: 事件 tap 未创建(权限?)" >&2
[ -f "$SCRATCH/smoke.db" ] || { echo "FAIL: DB 未创建" >&2; exit 1; }

kill -TERM "$PID" 2>/dev/null || true
for _ in $(seq 1 50); do
  if ! kill -0 "$PID" 2>/dev/null; then break; fi
  sleep 0.2
done
if kill -0 "$PID" 2>/dev/null; then
  echo "FAIL: SIGTERM 后未退出" >&2
  kill -KILL "$PID" 2>/dev/null || true
  exit 1
fi
wait "$PID" 2>/dev/null || true
echo "=== 冒烟通过(菜单/状态栏/启动/tap/DB/退出) ==="
