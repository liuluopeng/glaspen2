part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分; extension 里 setState 属预期用法。
// ignore_for_file: invalid_use_of_protected_member

// ── 可配置快捷键: 动作表 + 按键录制 ──
//
// 后端(macOS)只认规范化 chord: cmd+ctrl+alt+shift+<Key>，顺序固定,
// 至少包含 ctrl 或 alt; 空串 = 未绑定。这里负责展示、录制、冲突校验。
// 录制期间通过 setSetting('hotkeyRecording', true) 暂停全局事件拦截,
// 否则面板按下的组合会被 overlay 先吃掉。

class _ShortcutAction {
  final String id;
  final String zh;
  final String en;
  final String defaultChord;
  const _ShortcutAction(this.id, this.zh, this.en, this.defaultChord);
}

const List<_ShortcutAction> _shortcutActions = [
  _ShortcutAction('undo', '撤销上一笔', 'Undo', 'cmd+ctrl+Z'),
  _ShortcutAction('page_prev', '上一页', 'Previous page', 'cmd+ctrl+`'),
  _ShortcutAction('page_next', '下一页', 'Next page', 'cmd+ctrl+1'),
  _ShortcutAction('color_swap', '换回上一颜色', 'Swap color', 'cmd+ctrl+E'),
  _ShortcutAction('width_swap', '换回上一粗细', 'Swap width', 'cmd+ctrl+W'),
  _ShortcutAction('toggle_draw', '启用/停用涂鸦', 'Toggle doodle', 'cmd+ctrl+V'),
  _ShortcutAction('clear', '新建画布(清空)', 'New canvas (clear)', 'cmd+ctrl+C'),
  _ShortcutAction('export', '导出 SVG + GIF', 'Export SVG + GIF', 'cmd+ctrl+G'),
  _ShortcutAction('copy_svg', '复制 SVG', 'Copy SVG', 'cmd+ctrl+S'),
  _ShortcutAction('toggle_glass', '磨砂玻璃', 'Frosted glass', 'cmd+ctrl+B'),
  _ShortcutAction('toggle_ethereal', '飘渺画布', 'Ethereal canvas', 'cmd+ctrl+X'),
  _ShortcutAction('settings', '打开设置', 'Open settings', 'cmd+ctrl+D'),
];

/// 按住类功能/系统锁屏占用的组合, 普通动作不能选。
const Set<String> _reservedShortcuts = {
  'cmd+ctrl+Q',
  'cmd+ctrl+R',
  'cmd+ctrl+2',
  'cmd+ctrl+3',
};

/// USB HID usage(去掉 page, 即 & 0xffff) → 后端规范键名。
/// 字母/数字/标点走 US 物理位置; 与后端 gl_keycode_names() 一一对应。
const Map<int, String> _hidKeyNames = {
  0x04: 'A', 0x05: 'B', 0x06: 'C', 0x07: 'D', 0x08: 'E', 0x09: 'F',
  0x0A: 'G', 0x0B: 'H', 0x0C: 'I', 0x0D: 'J', 0x0E: 'K', 0x0F: 'L',
  0x10: 'M', 0x11: 'N', 0x12: 'O', 0x13: 'P', 0x14: 'Q', 0x15: 'R',
  0x16: 'S', 0x17: 'T', 0x18: 'U', 0x19: 'V', 0x1A: 'W', 0x1B: 'X',
  0x1C: 'Y', 0x1D: 'Z',
  0x1E: '1', 0x1F: '2', 0x20: '3', 0x21: '4', 0x22: '5', 0x23: '6',
  0x24: '7', 0x25: '8', 0x26: '9', 0x27: '0',
  0x28: 'Return', 0x29: 'Escape', 0x2A: 'Backspace', 0x2B: 'Tab',
  0x2C: 'Space',
  0x2D: '-', 0x2E: '=', 0x2F: '[', 0x30: ']', 0x31: r'\', 0x33: ';',
  0x34: "'", 0x35: '`', 0x36: ',', 0x37: '.', 0x38: '/',
  0x3A: 'F1', 0x3B: 'F2', 0x3C: 'F3', 0x3D: 'F4', 0x3E: 'F5', 0x3F: 'F6',
  0x40: 'F7', 0x41: 'F8', 0x42: 'F9', 0x43: 'F10', 0x44: 'F11', 0x45: 'F12',
  0x4A: 'Home', 0x4B: 'PageUp', 0x4C: 'Delete', 0x4D: 'End',
  0x4E: 'PageDown', 0x4F: 'Right', 0x50: 'Left', 0x51: 'Down', 0x52: 'Up',
};

final Set<LogicalKeyboardKey> _shortcutModifierKeys = {
  LogicalKeyboardKey.control,
  LogicalKeyboardKey.controlLeft,
  LogicalKeyboardKey.controlRight,
  LogicalKeyboardKey.shift,
  LogicalKeyboardKey.shiftLeft,
  LogicalKeyboardKey.shiftRight,
  LogicalKeyboardKey.alt,
  LogicalKeyboardKey.altLeft,
  LogicalKeyboardKey.altRight,
  LogicalKeyboardKey.meta,
  LogicalKeyboardKey.metaLeft,
  LogicalKeyboardKey.metaRight,
};

/// settings_json 的 shortcuts 字段 → 完整表(缺失动作补默认)。
Map<String, String> _decodeShortcuts(dynamic raw) {
  final out = <String, String>{
    for (final a in _shortcutActions) a.id: a.defaultChord,
  };
  dynamic decoded = raw;
  if (raw is String && raw.trim().isNotEmpty) {
    try {
      decoded = jsonDecode(raw);
    } catch (_) {
      // 损坏的存档: 保持默认表
    }
  }
  if (decoded is Map) {
    for (final a in _shortcutActions) {
      final v = decoded[a.id];
      if (v is String) out[a.id] = v;
    }
  }
  return out;
}

String _prettyShortcut(String chord, bool en) {
  if (chord.isEmpty) return en ? 'Unbound' : '未绑定';
  const symbols = <String, String>{
    'cmd': '⌘',
    'ctrl': '⌃',
    'alt': '⌥',
    'shift': '⇧',
    'Up': '↑',
    'Down': '↓',
    'Left': '←',
    'Right': '→',
    'PageUp': 'PgUp',
    'PageDown': 'PgDn',
    'Return': '↩',
    'Escape': 'Esc',
    'Backspace': '⌫',
    'Delete': '⌦',
    'Tab': '⇥',
    'Space': 'Space',
  };
  return chord.split('+').map((p) => symbols[p] ?? p).join();
}

/// 校验录制结果; null = 可用。空串(清除)永远可用。
String? _shortcutError(
    String chord, String actionId, Map<String, String> map, bool en) {
  if (chord.isEmpty) return null;
  final parts = chord.split('+');
  final mods = parts.length > 1 ? parts.sublist(0, parts.length - 1) : const <String>[];
  if (!_hidKeyNames.values.contains(parts.last)) {
    return en ? 'Unsupported key' : '不支持的按键';
  }
  if (!mods.contains('ctrl') && !mods.contains('alt')) {
    return en ? 'Must include Ctrl or Option' : '需要同时按 Ctrl 或 Option';
  }
  if (_reservedShortcuts.contains(chord)) {
    return en ? 'Reserved by system/hold features' : '已被系统或按住类功能保留';
  }
  for (final a in _shortcutActions) {
    if (a.id == actionId) continue;
    if (map[a.id] == chord) {
      return en ? 'Conflicts with "${a.en}"' : '与「${a.zh}」冲突';
    }
  }
  return null;
}

String? _captureShortcutChord(KeyDownEvent event) {
  final usage = event.physicalKey.usbHidUsage & 0xffff;
  final key = _hidKeyNames[usage];
  if (key == null) return null;
  final mods = <String>[
    if (HardwareKeyboard.instance.isMetaPressed) 'cmd',
    if (HardwareKeyboard.instance.isControlPressed) 'ctrl',
    if (HardwareKeyboard.instance.isAltPressed) 'alt',
    if (HardwareKeyboard.instance.isShiftPressed) 'shift',
  ];
  mods.add(key);
  return mods.join('+');
}

extension _SettingsShortcutsUi on _SettingsPageState {
  String _shortcutText(String zh, String en) => _language == 'en' ? en : zh;

  String _shortcutSectionTitle() =>
      _shortcutText('快捷键', 'Shortcuts');

  Widget _buildShortcutEditor() {
    if (Platform.isWindows) {
      return Text(
        _shortcutText('快捷键编辑目前仅支持 macOS。',
            'Shortcut editing is currently macOS-only.'),
        style: TextStyle(fontSize: 13, color: Colors.grey.shade600),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          _shortcutText(
            '点击「修改」后按下组合键；需包含 Ctrl 或 Option，清空可取消绑定。',
            'Click Edit, then press a shortcut. Ctrl or Option is required; clear to unbind.',
          ),
          style: TextStyle(fontSize: 12, color: Colors.grey.shade600),
        ),
        const SizedBox(height: 4),
        Text(
          _shortcutText(
            '按住类快捷键（GIF / 手写消息）和无限画布方向键不在此列表。',
            'Hold shortcuts (GIF / messages) and infinite-canvas keys are not listed here.',
          ),
          style: TextStyle(fontSize: 11, color: Colors.grey.shade500),
        ),
        const SizedBox(height: 8),
        ..._shortcutActions.map(_shortcutRow),
        Align(
          alignment: Alignment.centerRight,
          child: TextButton.icon(
            onPressed: _resetShortcuts,
            icon: const Icon(Icons.restart_alt, size: 16),
            label: Text(_shortcutText('恢复默认', 'Reset defaults')),
          ),
        ),
      ],
    );
  }

  Widget _shortcutRow(_ShortcutAction a) {
    final chord = _shortcuts[a.id] ?? a.defaultChord;
    final label = _language == 'en' ? a.en : a.zh;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          Expanded(child: Text(label, style: const TextStyle(fontSize: 13))),
          InkWell(
            onTap: () => _recordShortcut(a.id),
            borderRadius: BorderRadius.circular(5),
            child: Container(
              constraints: const BoxConstraints(minWidth: 96),
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
              decoration: BoxDecoration(
                color: _paperCard,
                borderRadius: BorderRadius.circular(5),
                border: Border.all(color: _paperLine),
              ),
              alignment: Alignment.center,
              child: Text(
                _prettyShortcut(chord, _language == 'en'),
                style: TextStyle(
                  fontSize: 12,
                  fontFamily: 'Menlo',
                  color: chord.isEmpty ? Colors.grey.shade500 : _ink,
                ),
              ),
            ),
          ),
          const SizedBox(width: 4),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: _shortcutText('修改', 'Edit'),
            icon: const Icon(Icons.keyboard_outlined, size: 18),
            onPressed: () => _recordShortcut(a.id),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            tooltip: _shortcutText('清除', 'Clear'),
            icon: const Icon(Icons.close, size: 16),
            onPressed:
                chord.isEmpty ? null : () => _saveShortcuts({..._shortcuts, a.id: ''}),
          ),
        ],
      ),
    );
  }

  Future<void> _recordShortcut(String actionId) async {
    final action = _shortcutActions.firstWhere((a) => a.id == actionId);
    final initial = _shortcuts[actionId] ?? action.defaultChord;
    try {
      await _bridge.setSetting('hotkeyRecording', true);
    } catch (e) {
      debugPrint('[Shortcuts] pause hotkeys failed: $e');
      return;
    }
    String? result;
    try {
      if (!mounted) return;
      result = await showDialog<String>(
        context: context,
        barrierDismissible: false,
        builder: (_) => _ShortcutRecorderDialog(
          actionId: actionId,
          initial: initial,
          shortcuts: Map<String, String>.of(_shortcuts),
          english: _language == 'en',
        ),
      );
    } finally {
      // 等恢复完成再返回, 避免紧接着的第二次录制与这个 false 竞态。
      try {
        await _bridge.setSetting('hotkeyRecording', false);
      } catch (e) {
        debugPrint('[Shortcuts] resume hotkeys failed: $e');
      }
    }
    if (result == null || !mounted) return;
    _saveShortcuts({..._shortcuts, actionId: result});
  }

  void _saveShortcuts(Map<String, String> next) {
    setState(() => _shortcuts = Map<String, String>.of(next));
    _settingsLoadGen++; // 用户刚改的优先于任何在途旧加载
    unawaited(_bridge.setSetting('shortcuts', next).catchError((Object e) {
      debugPrint('[Shortcuts] save failed: $e');
    }));
  }

  void _resetShortcuts() {
    _saveShortcuts({
      for (final a in _shortcutActions) a.id: a.defaultChord,
    });
  }
}

class _ShortcutRecorderDialog extends StatefulWidget {
  final String actionId;
  final String initial;
  final Map<String, String> shortcuts;
  final bool english;
  const _ShortcutRecorderDialog({
    required this.actionId,
    required this.initial,
    required this.shortcuts,
    required this.english,
  });

  @override
  State<_ShortcutRecorderDialog> createState() => _ShortcutRecorderDialogState();
}

class _ShortcutRecorderDialogState extends State<_ShortcutRecorderDialog> {
  late String _captured = widget.initial;
  String? _error;
  final FocusNode _focusNode = FocusNode(debugLabel: 'shortcut-recorder');

  @override
  void initState() {
    super.initState();
    _error = _shortcutError(
        _captured, widget.actionId, widget.shortcuts, widget.english);
    // AlertDialog 可能把初始焦点给按钮; 下一帧把焦点抢回录制区。
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  String _t(String zh, String en) => widget.english ? en : zh;

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is KeyRepeatEvent) return KeyEventResult.handled;
    if (event is! KeyDownEvent) return KeyEventResult.handled;
    final mods = <String>[
      if (HardwareKeyboard.instance.isMetaPressed) 'cmd',
      if (HardwareKeyboard.instance.isControlPressed) 'ctrl',
      if (HardwareKeyboard.instance.isAltPressed) 'alt',
      if (HardwareKeyboard.instance.isShiftPressed) 'shift',
    ];
    if (event.logicalKey == LogicalKeyboardKey.escape && mods.isEmpty) {
      Navigator.of(context).pop();
      return KeyEventResult.handled;
    }
    if (_shortcutModifierKeys.contains(event.logicalKey)) {
      return KeyEventResult.handled; // 等主键; 不把按住修饰键当错误
    }
    final chord = _captureShortcutChord(event);
    if (chord == null) {
      setState(() => _error = _t('不支持这个按键', 'Unsupported key'));
      return KeyEventResult.handled;
    }
    setState(() {
      _captured = chord;
      _error = _shortcutError(
          chord, widget.actionId, widget.shortcuts, widget.english);
    });
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final canSave = _captured.isNotEmpty && _error == null;
    return AlertDialog(
      title: Text(_t('修改快捷键', 'Edit shortcut')),
      content: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(_t('按下组合键（需包含 Ctrl 或 Option）',
                'Press a shortcut (Ctrl or Option required)')),
            const SizedBox(height: 12),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(6),
              ),
              alignment: Alignment.center,
              child: Text(
                _prettyShortcut(_captured, widget.english),
                style: const TextStyle(
                  fontSize: 18,
                  fontFamily: 'Menlo',
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(_error!,
                  style: const TextStyle(color: Colors.red, fontSize: 12)),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(_t('取消', 'Cancel')),
        ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(''),
          child: Text(_t('清除', 'Clear')),
        ),
        FilledButton(
          onPressed: canSave ? () => Navigator.of(context).pop(_captured) : null,
          child: Text(_t('保存', 'Save')),
        ),
      ],
    );
  }
}
