part of 'main.dart';

// 面板状态恢复在 extension 里访问 State.mounted 属预期用法(同 main.settings_state.dart)。
// ignore_for_file: invalid_use_of_protected_member

// ── 面板 UI 状态持久化(跨启动记住上次停留的 tab)──
//
// 这是纯面板偏好, 不进 core settings 契约(无需改 Rust/Swift/管道三端)。
// 位置与数据库同目录: 优先 GLASPEN2_DB_PATH 的父目录(测试隔离), 否则沿用
// 平台的应用支持目录。写入失败只 debugPrint, 不影响面板功能。

/// 上次停留的 tab 记录文件名。
const String _panelStateFileName = 'panel_state.json';

File? _panelStateFile() {
  final dir = panelStateDirPath();
  if (dir == null) return null;
  return File('$dir/$_panelStateFileName');
}

/// 记录当前 tab(异步 fire-and-forget;失败不打断 UI)。
void _savePanelTab(int index) {
  final file = _panelStateFile();
  if (file == null) return;
  unawaited(() async {
    try {
      await file.parent.create(recursive: true);
      await file.writeAsString(jsonEncode({'tab': index}));
    } catch (e) {
      debugPrint('[PanelState] write failed: $e');
    }
  }());
}

extension _PanelTabUi on _SettingsPageState {
  /// 同步读(文件很小): 初始 tab 必须在设置快照到达前就绪, 否则设置先到会
  /// 走"跟随画布模式"的兜底, 用户的记忆值就永远用不上。
  void _restorePanelTabSync() {
    final file = _panelStateFile();
    if (file != null && file.existsSync()) {
      try {
        final decoded = jsonDecode(file.readAsStringSync());
        if (decoded is Map && decoded['tab'] is int) {
          _savedTabIndex = decoded['tab'] as int;
        }
      } catch (e) {
        debugPrint('[PanelState] read failed: $e');
      }
    }
    _panelTabLoaded = true;
    _applyInitialTabIfReady();
  }

  /// 设置快照与 panel_state 都就绪后决定初始 tab(只执行一次)。
  /// 用户已经手动切过 tab 时不再干预。
  void _applyInitialTabIfReady() {
    if (_initialTabApplied ||
        !_settingsReadyForInitialTab ||
        !_panelTabLoaded) {
      return;
    }
    _initialTabApplied = true;
    if (!mounted || _userTabTouched) return;
    final saved = _savedTabIndex;
    if (saved != null && saved >= 0 && saved < _tabController.length) {
      if (_tabController.index != saved) {
        _syncingTabs = true;
        _tabController.index = saved;
        _syncingTabs = false;
        _afterTabActivated(saved);
      }
      return;
    }
    // 无记录 / tab 已不存在(自由涂鸦关闭等): 保持旧行为, 跟随画布模式。
    _syncTabToCanvasMode(force: true);
  }
}
