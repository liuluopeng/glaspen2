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
  // ⌘⌃W 与微信「显示/隐藏主面板」默认冲突; T = thickness。
  _ShortcutAction('width_swap', '换回上一粗细', 'Swap width', 'cmd+ctrl+T'),
  _ShortcutAction('toggle_draw', '启用/停用涂鸦', 'Toggle doodle', 'cmd+ctrl+V'),
  _ShortcutAction('toggle_free_canvas', '自由涂鸦 (无限画布)', 'Free doodle (infinite canvas)', 'cmd+ctrl+I'),
  // 左手镜头: 拇指按住左 ⌥, 手指留在 WASD; Z/X 缩放。
  _ShortcutAction('canvas_pan_left', '自由涂鸦: 镜头左移', 'Free doodle: pan left', 'alt+A'),
  _ShortcutAction('canvas_pan_right', '自由涂鸦: 镜头右移', 'Free doodle: pan right', 'alt+D'),
  _ShortcutAction('canvas_pan_up', '自由涂鸦: 镜头上移', 'Free doodle: pan up', 'alt+W'),
  _ShortcutAction('canvas_pan_down', '自由涂鸦: 镜头下移', 'Free doodle: pan down', 'alt+S'),
  _ShortcutAction('canvas_zoom_in', '自由涂鸦: 放大', 'Free doodle: zoom in', 'alt+X'),
  _ShortcutAction('canvas_zoom_out', '自由涂鸦: 缩小', 'Free doodle: zoom out', 'alt+Z'),
  _ShortcutAction('clear', '新建画布(清空)', 'New canvas (clear)', 'cmd+ctrl+C'),
  _ShortcutAction('export', '导出 SVG + GIF', 'Export SVG + GIF', 'cmd+ctrl+G'),
  _ShortcutAction('copy_svg', '复制 SVG', 'Copy SVG', 'cmd+ctrl+S'),
  _ShortcutAction('toggle_glass', '磨砂玻璃', 'Frosted glass', 'cmd+ctrl+B'),
  _ShortcutAction('toggle_ethereal', '飘渺画布', 'Ethereal canvas', 'cmd+ctrl+X'),
  _ShortcutAction('settings', '打开设置', 'Open settings', 'cmd+ctrl+D'),
];

/// 系统/常用 App 默认与按住类功能占用的组合, 普通动作不能选。
/// W/A 是微信(macOS)的显示隐藏/截图默认; F 是 macOS 全屏。
const Set<String> _reservedShortcuts = {
  'cmd+ctrl+Q', // macOS 锁屏
  'cmd+ctrl+R', // 按住录 GIF
  'cmd+ctrl+2', // 按住手写草稿
  'cmd+ctrl+3', // 按住手写消息
  'cmd+ctrl+W', // 微信 显示/隐藏主面板
  'cmd+ctrl+A', // 微信 截图
  'cmd+ctrl+F', // macOS 全屏
};

/// 键帽右下角的一句话动作提示(旧版快捷键按钮的 label 信息)。
/// 取该键当前绑定的第一个动作; 同键多动作显示 "+N"。
const Map<String, String> _shortcutHintsZh = {
  'undo': '撤销',
  'page_prev': '上页',
  'page_next': '下页',
  'color_swap': '换色',
  'width_swap': '换粗细',
  'toggle_draw': '涂鸦',
  'toggle_free_canvas': '自由',
  'canvas_pan_left': '左移',
  'canvas_pan_right': '右移',
  'canvas_pan_up': '上移',
  'canvas_pan_down': '下移',
  'canvas_zoom_in': '放大',
  'canvas_zoom_out': '缩小',
  'clear': '清空',
  'export': '导出',
  'copy_svg': '复制',
  'toggle_glass': '玻璃',
  'toggle_ethereal': '飘渺',
  'settings': '设置',
};

const Map<String, String> _shortcutHintsEn = {
  'undo': 'Undo',
  'page_prev': 'Prev',
  'page_next': 'Next',
  'color_swap': 'Color',
  'width_swap': 'Width',
  'toggle_draw': 'Doodle',
  'toggle_free_canvas': 'Free',
  'canvas_pan_left': 'Left',
  'canvas_pan_right': 'Right',
  'canvas_pan_up': 'Up',
  'canvas_pan_down': 'Down',
  'canvas_zoom_in': 'In',
  'canvas_zoom_out': 'Out',
  'clear': 'Clear',
  'export': 'Export',
  'copy_svg': 'Copy',
  'toggle_glass': 'Glass',
  'toggle_ethereal': 'Mist',
  'settings': 'Setup',
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
    return en
        ? 'Reserved by macOS, common apps, or hold features'
        : '已被系统、常用 App 或按住类功能保留';
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

  /// 当前所有绑定里用到的修饰键/主键, 供键盘示意图高亮。
  Widget _buildShortcutKeyboard() {
    final usedModifiers = <String>{};
    final usedKeys = <String>{};
    for (final chord in _shortcuts.values) {
      if (chord.isEmpty) continue;
      final parts = chord.split('+');
      if (parts.length < 2) continue;
      for (final m in parts.sublist(0, parts.length - 1)) {
        usedModifiers.add(m);
      }
      usedKeys.add(parts.last);
    }
    final keyboardNames = <String>{
      for (final row in _keyboardRows)
        for (final key in row) key.name,
    };
    final unmapped = usedKeys.difference(keyboardNames);
    // 键 → 当前绑定动作 id(保持动作表顺序, 第一个作为角标主文案)
    final keyActions = <String, List<String>>{};
    for (final action in _shortcutActions) {
      final chord = _shortcuts[action.id] ?? action.defaultChord;
      if (chord.isEmpty) continue;
      final parts = chord.split('+');
      if (parts.length < 2) continue;
      keyActions.putIfAbsent(parts.last, () => []).add(action.id);
    }
    final keyHints = <String, String>{};
    keyActions.forEach((key, ids) {
      final first = ids.first;
      final label = _language == 'en'
          ? (_shortcutHintsEn[first] ?? first)
          : (_shortcutHintsZh[first] ?? first);
      keyHints[key] = ids.length > 1 ? '$label+${ids.length - 1}' : label;
    });
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(
              _shortcutText('当前组合示意', 'Current bindings'),
              style: const TextStyle(
                  fontSize: 12, fontWeight: FontWeight.w600, color: _inkFaint),
            ),
            const Spacer(),
            Container(
              width: 9,
              height: 9,
              decoration: BoxDecoration(
                color: _penRed,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            const SizedBox(width: 5),
            Text(
              _shortcutText('已绑定', 'Bound'),
              style: const TextStyle(fontSize: 10, color: _inkFaint),
            ),
          ],
        ),
        const SizedBox(height: 6),
        RepaintBoundary(
          key: const ValueKey('shortcut-keyboard'),
          child: SizedBox(
            width: double.infinity,
            child: AspectRatio(
              aspectRatio: _keyboardAspect,
              child: CustomPaint(
                painter: _ShortcutKeyboardPainter(
                  usedModifiers: usedModifiers,
                  usedKeys: usedKeys,
                  keyHints: keyHints,
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 4),
        Text(
          _shortcutText(
            '键帽右下角 = 该键当前绑定的动作。',
            'Bottom-right of a key = its current action.',
          ),
          style: const TextStyle(fontSize: 10, color: _inkFaint),
        ),
        if (unmapped.isNotEmpty) ...[
          const SizedBox(height: 2),
          Text(
            _shortcutText(
              '方向键 / F 键等不在左侧示意图中。',
              'Arrow / F keys are not drawn in this left-side diagram.',
            ),
            style: const TextStyle(fontSize: 10, color: _inkFaint),
          ),
        ],
      ],
    );
  }

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
            '按住类快捷键（GIF / 手写消息）不在此列表；自由涂鸦的镜头平移/缩放在列表中。',
            'Hold shortcuts (GIF / messages) are not listed; free-doodle camera pan/zoom are.',
          ),
          style: TextStyle(fontSize: 11, color: Colors.grey.shade500),
        ),
        const SizedBox(height: 10),
        _buildShortcutKeyboard(),
        const SizedBox(height: 14),
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

// ── 左侧键盘示意图 ─────────────────────────────────────────────────
// 只画物理左侧一块(esc/数字行/Tab/字母/Caps/Shift/修饰键/Space),
// 按键名与后端 chord 的规范键名一致; 修饰键名 = cmd/ctrl/alt/shift。
// 这些键宽以 1u 为单位, 行内累计; 总宽 16u, 与右侧空格对齐。

class _KeyCap {
  final String label;
  final String name;
  final double width;
  final bool isModifier;
  const _KeyCap(this.label, this.name, this.width,
      {this.isModifier = false});
}

const List<List<_KeyCap>> _keyboardRows = [
  [
    _KeyCap('esc', 'Escape', 1.25),
    _KeyCap('`', '`', 1),
    _KeyCap('1', '1', 1),
    _KeyCap('2', '2', 1),
    _KeyCap('3', '3', 1),
    _KeyCap('4', '4', 1),
    _KeyCap('5', '5', 1),
    _KeyCap('6', '6', 1),
    _KeyCap('7', '7', 1),
    _KeyCap('8', '8', 1),
    _KeyCap('9', '9', 1),
    _KeyCap('0', '0', 1),
    _KeyCap('-', '-', 1),
    _KeyCap('=', '=', 1),
    _KeyCap('delete', 'Backspace', 1.75),
  ],
  [
    _KeyCap('tab', 'Tab', 1.5),
    _KeyCap('Q', 'Q', 1),
    _KeyCap('W', 'W', 1),
    _KeyCap('E', 'E', 1),
    _KeyCap('R', 'R', 1),
    _KeyCap('T', 'T', 1),
    _KeyCap('Y', 'Y', 1),
    _KeyCap('U', 'U', 1),
    _KeyCap('I', 'I', 1),
    _KeyCap('O', 'O', 1),
    _KeyCap('P', 'P', 1),
    _KeyCap('[', '[', 1),
    _KeyCap(']', ']', 1),
    _KeyCap('\\', '\\', 1.5),
  ],
  [
    _KeyCap('caps', 'CapsLock', 1.75),
    _KeyCap('A', 'A', 1),
    _KeyCap('S', 'S', 1),
    _KeyCap('D', 'D', 1),
    _KeyCap('F', 'F', 1),
    _KeyCap('G', 'G', 1),
    _KeyCap('H', 'H', 1),
    _KeyCap('J', 'J', 1),
    _KeyCap('K', 'K', 1),
    _KeyCap('L', 'L', 1),
    _KeyCap(';', ';', 1),
    _KeyCap("'", "'", 1),
    _KeyCap('return', 'Return', 2.25),
  ],
  [
    _KeyCap('shift', 'shift', 2.25, isModifier: true),
    _KeyCap('Z', 'Z', 1),
    _KeyCap('X', 'X', 1),
    _KeyCap('C', 'C', 1),
    _KeyCap('V', 'V', 1),
    _KeyCap('B', 'B', 1),
    _KeyCap('N', 'N', 1),
    _KeyCap('M', 'M', 1),
    _KeyCap(',', ',', 1),
    _KeyCap('.', '.', 1),
    _KeyCap('/', '/', 1),
    _KeyCap('shift', 'shift', 2.75, isModifier: true),
  ],
  [
    _KeyCap('ctrl', 'ctrl', 1.25, isModifier: true),
    _KeyCap('opt', 'alt', 1.25, isModifier: true),
    _KeyCap('cmd', 'cmd', 1.25, isModifier: true),
    _KeyCap('space', 'Space', 5.0),
    _KeyCap('cmd', 'cmd', 1.25, isModifier: true),
    _KeyCap('opt', 'alt', 1.25, isModifier: true),
    _KeyCap('ctrl', 'ctrl', 1.25, isModifier: true),
  ],
];

/// 宽 16u + 左右各 0.15u 内边距; 高 5 行 × (0.9u + 0.1u) + 上下 0.15u。
const double _keyboardAspect = 16.3 / 5.2;

class _ShortcutKeyboardPainter extends CustomPainter {
  final Set<String> usedModifiers;
  final Set<String> usedKeys;
  final Map<String, String> keyHints;
  const _ShortcutKeyboardPainter({
    required this.usedModifiers,
    required this.usedKeys,
    required this.keyHints,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final u = size.width / 16.3;
    final pad = u * 0.15;
    final keyH = u * 0.9;
    final gap = u * 0.06;
    final rowGap = u * 0.1;

    final body = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.circular(u * 0.28),
    );
    canvas.drawRRect(
      body,
      Paint()..color = _paperLine.withValues(alpha: 0.55),
    );
    canvas.drawRRect(
      body.deflate(u * 0.04),
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = u * 0.03
        ..color = Colors.white.withValues(alpha: 0.55),
    );

    double y = pad;
    for (final row in _keyboardRows) {
      double x = pad;
      for (final key in row) {
        final w = key.width * u - gap;
        final rect = Rect.fromLTWH(x, y, w, keyH);
        final rr = RRect.fromRectAndRadius(
          rect,
          Radius.circular(u * 0.12),
        );
        final active = key.isModifier
            ? usedModifiers.contains(key.name)
            : usedKeys.contains(key.name);

        // keycap 底部投影
        canvas.drawRRect(
          rr.shift(Offset(0, u * 0.05)),
          Paint()..color = Colors.black.withValues(alpha: 0.10),
        );

        final fill = Paint()..style = PaintingStyle.fill;
        if (active) {
          // 修饰键是"按住的键", 实心红; 普通键可能很多, 用浅红底
          // + 红描边/红字, 避免整块键盘糊成一片红。
          fill.color = key.isModifier
              ? _penRed
              : _penRed.withValues(alpha: 0.16);
        } else {
          fill.shader = LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: key.isModifier
                ? const [Color(0xFFFFFDF8), Color(0xFFEDE6D7)]
                : const [Colors.white, Color(0xFFF5F1E7)],
          ).createShader(rect);
        }
        canvas.drawRRect(rr, fill);
        canvas.drawRRect(
          rr,
          Paint()
            ..style = PaintingStyle.stroke
            ..strokeWidth = u * 0.025
            ..color = active ? _penRed : _paperLine,
        );
        if (!active) {
          // 键帽顶沿高光, 让平面图有一点立体感
          canvas.drawLine(
            Offset(rect.left + u * 0.12, rect.top + u * 0.10),
            Offset(rect.right - u * 0.12, rect.top + u * 0.10),
            Paint()
              ..color = Colors.white.withValues(alpha: 0.7)
              ..strokeWidth = u * 0.03,
          );
        }

        final long = key.label.length > 2;
        final fs =
            (long ? u * 0.23 : u * 0.34).clamp(6.5, 13.0).toDouble();
        final tp = TextPainter(
          text: TextSpan(
            text: key.label,
            style: TextStyle(
              fontSize: fs,
              fontWeight: active ? FontWeight.w700 : FontWeight.w500,
              color: active
                  ? (key.isModifier ? Colors.white : _penRed)
                  : _ink,
            ),
          ),
          textDirection: TextDirection.ltr,
          maxLines: 1,
        )..layout();
        canvas.save();
        canvas.translate(
          rect.center.dx - tp.width / 2,
          rect.center.dy - tp.height / 2,
        );
        tp.paint(canvas, Offset.zero);
        canvas.restore();

        // 键帽右下角: 该键当前绑定的动作(旧版按钮 label 的角标版)
        final hint = key.isModifier ? null : keyHints[key.name];
        if (hint != null && hint.isNotEmpty) {
          final hintFs = (u * 0.18).clamp(6.0, 8.5).toDouble();
          final hp = TextPainter(
            text: TextSpan(
              text: hint,
              style: TextStyle(
                fontSize: hintFs,
                fontWeight: FontWeight.w600,
                color: _penRed.withValues(alpha: 0.86),
              ),
            ),
            textDirection: TextDirection.ltr,
            maxLines: 1,
            ellipsis: '…',
          )..layout(maxWidth: (w - u * 0.12).clamp(1.0, double.infinity));
          canvas.save();
          canvas.translate(
            rect.right - hp.width - u * 0.08,
            rect.bottom - hp.height - u * 0.06,
          );
          hp.paint(canvas, Offset.zero);
          canvas.restore();
        }

        x += key.width * u;
      }
      y += keyH + rowGap;
    }
  }

  @override
  bool shouldRepaint(covariant _ShortcutKeyboardPainter oldDelegate) => true;
}

