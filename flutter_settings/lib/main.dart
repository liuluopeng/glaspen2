import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';
import 'dart:math' as math;

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart'
    show Clipboard, ClipboardData, rootBundle, HardwareKeyboard, KeyEvent,
         KeyDownEvent, KeyRepeatEvent, LogicalKeyboardKey;
import 'package:flutter/foundation.dart';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'l10n/app_localizations.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart' as frb;

import 'src/rust/api.dart' as rust;
import 'src/rust/frb_generated.dart';

part 'main.bridge.dart';
part 'main.models.dart';
part 'main.canvas.dart';
part 'main.settings_state.dart';
part 'main.settings_schema.dart';
part 'main.notebook_data.dart';
part 'main.notebook.dart';
part 'main.update.dart';
part 'main.settings_widgets.dart';
part 'main.shortcuts.dart';

void main() {
  // FRB 需要绑定初始化(消息端口/isolate)后才能解析 Rust 符号
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const GlaspenSettingsApp());
}

/// 面板语言。默认中文, **不跟随系统自动检测** —— 由设置里的选择器决定。
/// 设置加载/原生菜单切换时更新这个 notifier, MaterialApp 立即重建。
final ValueNotifier<Locale> appLocale = ValueNotifier(const Locale('zh'));

// ── 后端下发的平台 UI 配置(Flutter 侧唯一的平台差异来源) ──
//
// 平台行为(默认键位/保留组合/强制绑定/修饰键名称/键盘示意图行/字号/功能
// 可用性)全部由后端 JSON 描述, schema 见 crates/glaspen-core/src/ui.rs:
// macOS 走 FRB uiConfigJson(), Windows 走命名管道 uiConfig。Flutter 代码
// 不再按平台写条件分支(由 test/platform_parity_test.dart 把关)。

/// 键盘示意图底部修饰键行里的一个键帽。
///
/// 注意 ui.rs 的字段语义: `name` 是键面显示文字(如 win/opt), `label` 是
/// chord 令牌(如 cmd/alt);与旧 Dart 表 `_KeyCap(label: 显示, name: 令牌)`
/// 的对应关系是 name→label、label→name。空间键同样是 name="space"、
/// label="Space"。ui.rs 里的注释把两者写反了(已知,已在交付说明里报告)。
class UiKeyCapSpec {
  final String name;
  final String label;
  final double width;
  final bool modifier;
  const UiKeyCapSpec({
    required this.name,
    required this.label,
    required this.width,
    required this.modifier,
  });

  static UiKeyCapSpec fromJson(dynamic raw) {
    if (raw is! Map) {
      return const UiKeyCapSpec(name: '', label: '', width: 1, modifier: false);
    }
    return UiKeyCapSpec(
      name: raw['name'] is String ? raw['name'] as String : '',
      label: raw['label'] is String ? raw['label'] as String : '',
      width: (raw['width'] as num?)?.toDouble() ?? 1.0,
      modifier: raw['modifier'] == true,
    );
  }
}

/// 键盘示意图底部修饰键行:left / space / right。
class UiModifierRow {
  final List<UiKeyCapSpec> left;
  final List<UiKeyCapSpec> space;
  final List<UiKeyCapSpec> right;
  const UiModifierRow({
    this.left = const [],
    this.space = const [],
    this.right = const [],
  });

  static List<UiKeyCapSpec> _keys(dynamic raw) {
    if (raw is! List) return const [];
    return [for (final e in raw) UiKeyCapSpec.fromJson(e)];
  }

  static UiModifierRow fromJson(dynamic raw) {
    if (raw is! Map) return const UiModifierRow();
    return UiModifierRow(
      left: _keys(raw['left']),
      space: _keys(raw['space']),
      right: _keys(raw['right']),
    );
  }
}

/// 后端 ui_config_json 的 Dart 侧模型(字段契约见 crates/glaspen-core/src/ui.rs)。
/// 一条设置项契约(来自 core/src/settings.rs 的登记表)。
/// 面板启动时用它自检后端快照是否漏项; 平台可用的项由后端 `mac/win` 标记决定,
/// Flutter 不猜平台。
class UiSettingSpec {
  final String key;
  final String dbKey;
  final String kind;
  final bool mac;
  final bool win;
  /// 表单 schema(D11a 起随设置契约下发); 非表单项(implicit/readOnly/hidden/
  /// protocolOnly)没有 section/labelId, 解析为 null。
  final UiFormMeta? form;
  const UiSettingSpec({
    required this.key,
    required this.dbKey,
    required this.kind,
    required this.mac,
    required this.win,
    this.form,
  });

  static List<UiSettingSpec> _list(dynamic raw) {
    if (raw is! List) return const [];
    final out = <UiSettingSpec>[];
    for (final e in raw) {
      if (e is! Map) continue;
      final key = e['key'];
      if (key is! String || key.isEmpty) continue;
      out.add(UiSettingSpec(
        key: key,
        dbKey: e['dbKey'] is String ? e['dbKey'] as String : key,
        kind: e['kind'] is String ? e['kind'] as String : 'str',
        mac: e['mac'] == true,
        win: e['win'] == true,
        form: UiFormMeta.tryParse(e['form']),
      ));
    }
    return out;
  }
}

class UiConfig {
  /// "macos" | "windows"
  final String platform;
  final bool isWindows;
  final double fontScale;
  /// chord 各段之间的显示分隔符(macOS 空串, Windows "+")。
  final String chordSeparator;
  /// chord 令牌 → 显示名(⌘/⌃/⌥/⇧ 或 Win/Ctrl/Alt/Shift)。
  final Map<String, String> chordNames;
  /// 文案里指代 ⌥/Alt 的那个词。
  final String modifierWord;
  /// 双语文案:key → {zh, en}。
  final Map<String, Map<String, String>> messages;
  /// 动作 id → 默认 chord(空串 = 默认不绑定)。
  final Map<String, String> defaults;
  /// 按住类固定热键:gif / draft / send。
  final Map<String, String> holdHotkeys;
  /// 系统/常用 App/按住类占用的组合,面板不允许选择。
  final List<String> reserved;
  /// 必须始终绑定、面板禁止清除的动作 id(Windows 没有菜单栏/托盘兜底)。
  final List<String> forceBound;
  /// 后端认定的动作集合与顺序(面板动作表必须与之一致)。
  final List<String> actionOrder;
  final UiModifierRow modifierRow;
  /// 功能可用性:invertInk / pageFlipEffect / pageExport / ocrSearch。
  final Map<String, bool> features;
  /// 设置项契约(core/settings.rs 登记表): 面板据此检查后端快照是否漏项。
  final List<UiSettingSpec> settings;
  /// 表单分区表(core/settings_form.rs): 顺序/标题 l10n 键/advanced/整节条件。
  final List<UiFormSection> settingsSections;
  /// 分区内非设置项的自定义区块(按钮/登录/关于等, Dart 按 id 手写)。
  final List<UiCustomBlock> settingsBlocks;
  /// 面板↔后端协议版本(core/protocol.rs); Windows 独立 exe 防新旧混跑。
  final int protocolVersion;

  const UiConfig({
    required this.platform,
    required this.isWindows,
    required this.fontScale,
    required this.chordSeparator,
    required this.chordNames,
    required this.modifierWord,
    required this.messages,
    required this.defaults,
    required this.holdHotkeys,
    required this.reserved,
    required this.forceBound,
    required this.actionOrder,
    required this.modifierRow,
    required this.features,
    this.settings = const [],
    this.settingsSections = const [],
    this.settingsBlocks = const [],
    this.protocolVersion = 0,
  });

  /// 该设置项在当前平台是否可用(登记表的 mac/win 标记)。
  bool settingAvailable(UiSettingSpec s) => isWindows ? s.win : s.mac;

  /// 解析后端 JSON;字段缺失/形状不对时抛 [FormatException],由调用方回退。
  factory UiConfig.fromJson(Map<String, dynamic> json) {
    final defaults = _uiStringMap(json['defaults']);
    final actionOrder = _uiStringList(json['actionOrder']);
    if (defaults.isEmpty || actionOrder.isEmpty) {
      throw FormatException(
          'uiConfig 缺少 defaults/actionOrder(管道未就绪或后端版本不匹配)');
    }
    return UiConfig(
      platform: json['platform'] is String ? json['platform'] as String : '',
      isWindows: json['isWindows'] == true,
      fontScale: _uiDouble(json['fontScale'], 1.0),
      chordSeparator:
          json['chordSeparator'] is String ? json['chordSeparator'] as String : '',
      chordNames: _uiStringMap(json['chordNames']),
      modifierWord:
          json['modifierWord'] is String ? json['modifierWord'] as String : 'Alt',
      messages: _uiMessages(json['messages']),
      defaults: defaults,
      holdHotkeys: _uiStringMap(json['holdHotkeys']),
      reserved: _uiStringList(json['reserved']),
      forceBound: _uiStringList(json['forceBound']),
      actionOrder: actionOrder,
      modifierRow: UiModifierRow.fromJson(json['modifierRow']),
      features: _uiBoolMap(json['features']),
      settings: UiSettingSpec._list(json['settings']),
      settingsSections: UiFormSection.list(json['settingsSections']),
      settingsBlocks: UiCustomBlock.list(json['settingsBlocks']),
      protocolVersion: (json['protocolVersion'] as num?)?.toInt() ?? 0,
    );
  }

  /// 安全兜底(等同一份 macOS 行为): widget 测试 / 管道暂时连不上时仍能渲染。
  /// 只用于降级,正常运行时必须以后端 JSON 为准。
  factory UiConfig.fallback() => const UiConfig(
        platform: 'macos',
        isWindows: false,
        fontScale: 1.4,
        chordSeparator: '',
        chordNames: {
          'cmd': '⌘', 'ctrl': '⌃', 'alt': '⌥', 'shift': '⇧',
          'Up': '↑', 'Down': '↓', 'Left': '←', 'Right': '→',
          'PageUp': 'PgUp', 'PageDown': 'PgDn',
          'Return': '↩', 'Escape': 'Esc', 'Backspace': '⌫', 'Delete': '⌦',
          'Tab': '⇥', 'Space': 'Space',
        },
        modifierWord: 'Option',
        messages: {
          'requireModifier': {
            'zh': '需要同时按 Ctrl 或 Option',
            'en': 'Must include Ctrl or Option',
          },
          'reserved': {
            'zh': '已被系统、常用 App 或按住类功能保留',
            'en': 'Reserved by macOS, common apps, or hold features',
          },
        },
        defaults: {
          'undo': 'cmd+ctrl+Z',
          'page_prev': 'cmd+ctrl+`',
          'page_next': 'cmd+ctrl+1',
          'color_swap': '',
          'width_swap': '',
          'toggle_draw': 'cmd+ctrl+V',
          'toggle_free_canvas': '',
          'canvas_pan_left': 'ctrl+alt+A',
          'canvas_pan_right': 'ctrl+alt+D',
          'canvas_pan_up': 'ctrl+alt+W',
          'canvas_pan_down': 'ctrl+alt+S',
          'canvas_zoom_in': 'ctrl+alt+X',
          'canvas_zoom_out': 'ctrl+alt+Z',
          'clear': '',
          'export': '',
          'copy_svg': '',
          'toggle_glass': '',
          'toggle_ethereal': '',
          'settings': 'cmd+ctrl+D',
          'quit': 'cmd+ctrl+shift+Q',
        },
        holdHotkeys: {
          'gif': 'cmd+ctrl+R',
          'draft': 'cmd+ctrl+2',
          'send': 'cmd+ctrl+3',
        },
        reserved: [
          'cmd+ctrl+Q',
          'cmd+ctrl+R',
          'cmd+ctrl+2',
          'cmd+ctrl+3',
          'cmd+ctrl+W',
          'cmd+ctrl+A',
          'cmd+ctrl+F',
        ],
        forceBound: [],
        actionOrder: [
          'undo', 'page_prev', 'page_next', 'color_swap', 'width_swap',
          'toggle_draw', 'toggle_free_canvas', 'canvas_pan_left',
          'canvas_pan_right', 'canvas_pan_up', 'canvas_pan_down',
          'canvas_zoom_in', 'canvas_zoom_out', 'clear', 'export', 'copy_svg',
          'toggle_glass', 'toggle_ethereal', 'settings', 'quit',
        ],
        modifierRow: UiModifierRow(
          left: [
            UiKeyCapSpec(name: 'ctrl', label: 'ctrl', width: 1.25, modifier: true),
            UiKeyCapSpec(name: 'opt', label: 'alt', width: 1.25, modifier: true),
            UiKeyCapSpec(name: 'cmd', label: 'cmd', width: 1.25, modifier: true),
          ],
          space: [
            UiKeyCapSpec(name: 'space', label: 'Space', width: 5.0, modifier: false),
          ],
          right: [
            UiKeyCapSpec(name: 'ctrl', label: 'ctrl', width: 1.25, modifier: true),
            UiKeyCapSpec(name: 'opt', label: 'alt', width: 1.25, modifier: true),
            UiKeyCapSpec(name: 'cmd', label: 'cmd', width: 1.25, modifier: true),
          ],
        ),
        features: {
          'invertInk': true,
          'pageFlipEffect': true,
          'pageExport': true,
          'ocrSearch': true,
        },
      );
}

Map<String, String> _uiStringMap(dynamic raw) {
  if (raw is! Map) return const {};
  final out = <String, String>{};
  raw.forEach((k, v) {
    if (v is String) out[k.toString()] = v;
  });
  return out;
}

List<String> _uiStringList(dynamic raw) {
  if (raw is! List) return const [];
  return [for (final v in raw) if (v is String) v];
}

Map<String, bool> _uiBoolMap(dynamic raw) {
  if (raw is! Map) return const {};
  final out = <String, bool>{};
  raw.forEach((k, v) {
    out[k.toString()] = v == true;
  });
  return out;
}

Map<String, Map<String, String>> _uiMessages(dynamic raw) {
  if (raw is! Map) return const {};
  final out = <String, Map<String, String>>{};
  raw.forEach((k, v) {
    out[k.toString()] = _uiStringMap(v);
  });
  return out;
}

double _uiDouble(dynamic v, double fallback) {
  if (v is num) return v.toDouble();
  if (v is String) return double.tryParse(v) ?? fallback;
  return fallback;
}

/// 当前生效的平台 UI 配置(加载完成前/失败时 = [UiConfig.fallback])。
final ValueNotifier<UiConfig> uiConfigNotifier =
    ValueNotifier<UiConfig>(UiConfig.fallback());

/// 顶层函数(快捷键格式化/校验)用的当前配置。
UiConfig get uiConfig => uiConfigNotifier.value;

// ── Platform-specific communication ──

const _pipeName = r'\\.\pipe\glaspen2_settings';

/// 「打开下载页」的兜底地址(GitHub 会重定向到最新 release;
/// 正常情况用检查结果里返回的 html_url)。
const _releasesPage = 'https://github.com/liuluopeng/glaspen2/releases/latest';

/// 缩略图批量块魔数 "GTH1"(与 Rust 侧 THUMB_BLOB_MAGIC 一致)。
const _thumbBlobMagic = 0x31485447;

/// 活页本缩略图尺寸:一条通道消息里可能带几十张,280 已足够清晰。
const _thumbMaxSize = 280;

/// 打开活页本时先取回多少页的缩略图(首屏 + 预取);其余滚动时按需批量补。
const _thumbInitialCount = 48;

/// 解析 Rust 侧 glaspen2_page_thumbnails / 管道 "blob" 的自描述二进制块:
/// magic u32 "GTH1",count u32,随后每项 id i64、len u32、PNG 字节(均小端)。
/// 返回的 Uint8List 是原缓冲的视图,不复制 PNG 数据。
Map<int, Uint8List> parseThumbnailBlob(Uint8List? blob) {
  if (blob == null || blob.length < 8) return const {};
  final view = ByteData.sublistView(blob);
  if (view.getUint32(0, Endian.little) != _thumbBlobMagic) return const {};
  final count = view.getUint32(4, Endian.little);
  final out = <int, Uint8List>{};
  var off = 8;
  for (var i = 0; i < count; i++) {
    if (off + 12 > blob.length) break; // 截断的块:保留已解析部分
    final id = view.getInt64(off, Endian.little);
    final len = view.getUint32(off + 8, Endian.little);
    off += 12;
    if (off + len > blob.length) break;
    out[id] = Uint8List.sublistView(blob, off, off + len);
    off += len;
  }
  return out;
}

/// 总览载荷里的 png:macOS 通道给 Uint8List,Windows 管道给 base64 字符串
Uint8List? _pngBytes(dynamic v) {
  if (v is Uint8List) return v;
  if (v is String && v.isNotEmpty) {
    try {
      return base64Decode(v);
    } catch (_) {
      return null;
    }
  }
  return null;
}

/// 服务器 JSON 里的开关是数字 0/1,统一转 bool
bool _b(dynamic v) => v == true || v == 1 || v == '1';

/// Abstract interface for settings communication.
///
/// macOS 用 flutter_rust_bridge 直接调同进程内的 Rust(`_FrbBridge`),
/// Windows 的面板是独立进程,只能走命名管道(`_NamedPipeBridge`)。
// 名称走 gen-l10n(zh/en 各一套), 颜色值只在一处。
List<String> _colorNamesOf(AppLocalizations l) =>
    [l.cRed, l.cOrange, l.cYellow, l.cGreen, l.cCyan, l.cBlue, l.cPurple, l.cPink, l.cWhite, l.cBlack];
const _colorValues = [
  0xFFFF0038, 0xFFFF4D00, 0xFFFFCC00, 0xFF00E676, 0xFF00A3FF,
  0xFF0096FF, 0xFFAA00FF, 0xFFFF0080, 0xFFFFFFFF, 0xFF000000,
];
List<String> _widthNamesOf(AppLocalizations l) =>
    [l.wUltraFine, l.wVeryFine, l.wFine, l.wMedium, l.wThick, l.wVeryThick, l.wExtraThick, l.wUltraThick];

class GlaspenSettingsApp extends StatelessWidget {
  const GlaspenSettingsApp({super.key, this.locale});

  /// 测试用固定语言; null = 跟随 [appLocale](设置里的选择)。
  final Locale? locale;


  @override
  Widget build(BuildContext context) {
    // 字号缩放读后端 uiConfig;加载完成后 notifier 触发整棵树重建。
    return ValueListenableBuilder<UiConfig>(
      valueListenable: uiConfigNotifier,
      builder: (context, cfg, _) {
        if (locale != null) return _build(context, locale!, cfg);
        return ValueListenableBuilder<Locale>(
          valueListenable: appLocale,
          builder: (context, l, _) => _build(context, l, cfg),
        );
      },
    );
  }

  Widget _build(BuildContext context, Locale locale, UiConfig cfg) {
    return MaterialApp(
      locale: locale,
      onGenerateTitle: (context) => AppLocalizations.of(context)!.glaspen2,
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      debugShowCheckedModeBanner: false,
      // macOS runs at native 1x (no display scaling), so the whole UI looks
      // small vs Windows — scale the entire UI up to match. Windows keeps 1.0
      // (the OS display scaling handles it).
      builder: (context, child) {
        // 整屏缩放由后端决定(macOS 1.4, Windows 1.0 = 交给系统缩放)
        final s = cfg.fontScale;
        Widget w = child!;
        if (s != 1.0) {
          w = Transform.scale(
            scale: s,
            child: FractionallySizedBox(
              widthFactor: 1 / s,
              heightFactor: 1 / s,
              child: w,
            ),
          );
        }
        // 纸底 + 点阵(Scaffold 背景透明,点阵从底层透出)
        return Container(
          color: _paperBg,
          child: CustomPaint(painter: const _PaperDotsPainter(), child: w),
        );
      },
      theme: _buildPaperTheme(),
      home: const SettingsPage(),
    );
  }
}

class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> with TickerProviderStateMixin {
  final _columnKey = GlobalKey();
  late SettingsBridge _bridge;
  late TabController _tabController;
  // ── 设置值(D11b: 表单的唯一数据源)──
  //
  // schema 渲染出的控件与迁移期的旧控件都读写这张表, 键 = 后端设置键;
  // 读取/加载/推送全部按 registry 的类型走, 不再为每个设置项加一个字段。
  // 初值 = 旧字段初值(与 core settings_snapshot 的缺省一致), 快照到达前
  // 面板就能渲染合法的表单。
  final Map<String, dynamic> _settingValues = <String, dynamic>{
    'color': 0,
    'width': 2,
    'pressureMonitor': false,
    'launchAtLogin': false,
    'grid': false,
    'gridFollowStrokes': false,
    'glassFollowStrokes': true,
    'softShadow': false,
    'invertInk': false,
    'invertFps': 30,
    'outline': false,
    'infiniteCanvas': false,
    'gridSize': 40, // 网格大小(逻辑 px)
    'gridDivider': 0, // 网格分栏:0=无 1=左右两栏 2=上下两栏 3=九宫格 4=康奈尔
    'minimap': false,
    'gifFps': 15,
    'gifResolution': 0.5,
    'gifSpeed': 2.0,
    'gifEndMode': 1, // 0=停在最后, 1=停1秒后循环, 2=立即循环
    'flipEffect': 0, // 翻页动效: 0=macOS 系统 1=时光隧道
    'chatIntegration': false, // 手写消息集成总开关(默认关)
    'showFreeCanvas': false,
    'showBubbleTab': false,
    'bubbleReplaySpeed': 1,
    'bubbleReplayHoldLast': true,
    'bubbleReplayAuto': false,
    'bubbleAutoNewPage': false,
    'shareCanvas': false,
    'notebookStyle': true, // 活页本外观: true=拟物玻璃 false=笔记纸
    'panelTransparent': false, // 真穿透(实验)
    'language': 'zh', // 界面语言: zh | en(不再跟随系统)
  };

  /// 设置值读取(JSON 数字的 int/double 差异在这里抹平)。
  bool _vBool(String key) => _settingValues[key] == true;
  int _vInt(String key, int fallback) {
    final v = _settingValues[key];
    return v is num ? v.toInt() : fallback;
  }

  double _vDouble(String key, double fallback) {
    final v = _settingValues[key];
    return v is num ? v.toDouble() : fallback;
  }

  String _vStr(String key) {
    final v = _settingValues[key];
    return v is String ? v : '';
  }

  /// 动态 UI(内容 tab/气泡重播/共享/旧控件)用的只读视图;
  /// 写值一律走 [_writeSetting] 或 [_setLanguage]。
  bool get _infiniteCanvas => _vBool('infiniteCanvas');
  bool get _showFreeCanvas => _vBool('showFreeCanvas');
  bool get _showBubbleTab => _vBool('showBubbleTab');
  bool get _notebookGlass => _vBool('notebookStyle');
  bool get _panelTransparent => _vBool('panelTransparent');
  String get _language => _vStr('language') == 'en' ? 'en' : 'zh';
  int get _selectedColor => _vInt('color', 0);
  int get _selectedWidth => _vInt('width', 2);
  int get _bubbleReplaySpeed => _vInt('bubbleReplaySpeed', 1).clamp(1, 8);
  bool get _bubbleReplayHoldLast => _vBool('bubbleReplayHoldLast');
  bool get _bubbleReplayAuto => _vBool('bubbleReplayAuto');
  int get _gifFps => _vInt('gifFps', 15);
  double get _gifResolution => _vDouble('gifResolution', 0.5);
  double get _gifSpeed => _vDouble('gifSpeed', 2.0);
  int get _gifEndMode => _vInt('gifEndMode', 1);
  bool get _minimapEnabled => _vBool('minimap');
  int get _flipEffect => _vInt('flipEffect', 0);

  // 可配置快捷键: action id → chord(空串 = 未绑定)。后端 settings_json 的
  // shortcuts 字段是这张表的 JSON 字符串; 面板负责展示/录制/校验。
  Map<String, String> _shortcuts = _decodeShortcuts(null);
  /// 后端 settings.shortcuts 原文: UiConfig 迟到加载时用它按新平台默认表重解码。
  dynamic _shortcutsRaw;
  // 缩略图按需加载:只加载可视区域的缩略图,滚动时按需补充
  final ScrollController _gridScroll = ScrollController();
  Timer? _gridScrollDebounce;
  // 画布总览 tab
  Uint8List? _canvasPng;
  List<double>? _canvasRect;
  /// 总览渲染的内容包围盒 [bx,by,bw,bh](画布坐标, 5% 外扩);
  /// 自由涂鸦编辑用它把位图坐标映射回画布。
  List<double> _canvasBounds = const [];
  bool _canvasLoading = false;
  // 自由涂鸦编辑(圈选/移动/复制粘贴/删除), screenId = -1 走无限画布
  bool _canvasEditing = false;
  ui.Image? _canvasEditImage;
  final List<Offset> _canvasLassoPts = [];
  List<int> _canvasSelectedStrokes = [];
  Offset? _canvasDragStart;
  Offset _canvasDragNow = Offset.zero;
  double _canvasEditScale = 1.0;
  Offset _canvasEditOffset = Offset.zero;
  final List<(List<int>, double, double)> _canvasUndoMoves = [];
  Timer? _reloadTimer;

  // 检查更新(「关于」区)
  String? _appVersion; // null = 还没取到(桥不可用时永远 null → 显示 —)
  bool _checkingUpdate = false;
  Map<dynamic, dynamic>? _updateResult;

  // 涂鸦身份(手写消息登录 axum;配置了才会给手写消息带作者)
  final _chatApiBase = TextEditingController();
  final _chatUser = TextEditingController();
  final _chatPassword = TextEditingController(); // 回显不填,留空 = 保持已存
  bool _chatHasPassword = false;
  String _chatLoginMsg = ''; // 测试登录的结果文案
  bool _chatLoginBusy = false;
  /// 程序性切换 tab 时置位: 避免同步 TabBar 选中项时又触发 _onTabChanged
  /// 反向发一轮 setSetting(会把画布模式来回切)。
  bool _syncingTabs = false;
  /// 首次拿到设置后是否已把 TabBar 落到当前画布模式对应的 tab。
  bool _initialTabApplied = false;
  /// 设置页渐进披露: 高级项收进「更多设置」, **默认展开**(用户明确要求:
  /// 打开设置就能看到外观/活页本/导出/备份/语言/快捷键, 不用先点一次)。
  bool _showAdvanced = true;
  bool _bubblesLoading = false;
  List<Map<String, dynamic>> _bubbleMsgs = const [];
  final ScrollController _bubbleScroll = ScrollController();
  int? _replayMsgId; // 正在重播的消息 id(null = 无)
  late final AnimationController _replayCtrl;
  // 「立即更新」状态机(检查到新版本、用户点"立即更新"并确认后才启动)
  _UpdPhase _upd = _UpdPhase.idle;
  int _updReceived = 0;
  int _updTotal = 0;
  String _updError = '';
  String _updTag = ''; // 确认对话框里确认过的 tag,stage 用
  StreamSubscription<Map<dynamic, dynamic>>? _updSub;

  // 10 colors, matching Rust COLOR_PRESETS / macOS g_color_presets
  // (红橙黄绿青蓝紫粉白黑). Index must match the overlay's preset order.
  // 同色相拉满亮度(S=100%,V≈100%): 亮/暗底同时显眼靠自动反色描边补足

  // Content tab state
  List<PageInfo> _pages = [];
  List<PageInfo> _filteredPages = [];
  /// 当前打开的笔记本("WxH"); null = 笔记本网格(本子列表)
  String? _openNotebook;
  // 页面详情(面板圈选/移动/复制粘贴/删除): 打开的页 id + 页面位图
  int? _detailPageId;
  ui.Image? _detailImage;
  double _detailImgW = 0, _detailImgH = 0;
  double _detailMargin = 0; // 页边距(页面坐标单位, 渲染时的留白)
  int _detailGen = 0; // 详情会话代数: 迟到的取图响应一律丢弃(防串页)
  final List<Offset> _lassoPts = [];
  List<int> _selectedStrokes = [];
  Offset? _dragStart;
  Offset _dragNow = Offset.zero;
  String _clipPayload = '';
  final List<(int, List<int>, double, double)> _undoMoves = [];
  int? _draggingPageId; // 活页本拖拽重排: 拖动中的页 id
  List<PageInfo> _dragOrderBackup = const []; // 拖拽开始时的顺序快照(失败回滚)
  double _detailScale = 1.0;
  Offset _detailOffset = Offset.zero;
  ui.Image? _tabBgImage; // 插画位图(真穿透时铺在网格底层, 卡片挖洞)
  /// OCR 搜索(本子页视图内): 激活后网格 = 全库匹配页(跨组)
  bool _searchMode = false;
  bool _searchFieldVisible = false;
  String _searchText = '';
  bool _pagesLoading = false;
  // 设置加载代数: 首帧 getSettings 还没回来时用户点了开关, 旧响应不能
  // 把新选择覆盖回去。
  int _settingsLoadGen = 0;
  // 批量多选删除
  bool _multiSelect = false;
  final Set<int> _selectedPageIds = {};
  bool _batchDeleting = false;
  final _thumbnailCache = <int, Uint8List>{};
  final _loadingThumbnails = <int>{};
  /// 待批量请求的页 id:同屏多次触发会合并成一次通道往返
  final _thumbQueue = <int>{};
  bool _batchRunning = false;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _tabController.addListener(() {
      if (!_tabController.indexIsChanging) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) _updateVisibleRange();
        });
      }
    });
    _tabController.addListener(_onTabChanged);
    _gridScroll.addListener(_onGridScroll);
    // 气泡重播: 播完按设置决定停在满帧还是停 600ms 后循环
    _replayCtrl = AnimationController(vsync: this)
      ..addStatusListener((status) {
        if (status != AnimationStatus.completed) return;
        if (!_bubbleReplayHoldLast && mounted) {
          Future.delayed(const Duration(milliseconds: 600), () {
            if (mounted && _replayMsgId != null && !_bubbleReplayHoldLast) {
              _replayCtrl.forward(from: 0);
            }
          });
        }
      });
    _bridge = createBridge();
    _bridge.onSettingsChanged(_onSettingsChanged);
    // Windows 管道连接成功后重新拉取设置与平台 UI 配置(macOS 通道立即可用)
    _bridge.onConnected = () {
      _loadSettings();
      unawaited(_loadUiConfig());
    };
    _loadSettings();
    unawaited(_loadUiConfig());
    // 「关于」区显示当前版本;取不到只显示 —,不影响面板其它功能
    unawaited(_loadAppVersion());
  }

  @override
  void dispose() {
    _gridScrollDebounce?.cancel();
    _gridScroll.removeListener(_onGridScroll);
    _gridScroll.dispose();
    _tabController.dispose();
    _bubbleScroll.dispose();
    _replayCtrl.dispose();
    _reloadTimer?.cancel();
    _updSub?.cancel(); // 还在下载时销毁面板 = 取消下载
    _chatApiBase.dispose();
    _chatUser.dispose();
    _chatPassword.dispose();
    _nasUrl.dispose();
    for (final c in _extraTextControllers.values) {
      c.dispose();
    }
    _bridge.dispose();
    super.dispose();
  }

  /// 写一个设置项: 更新本地值 → 通知后端 → 跑动态 UI 的副作用。
  ///
  /// 这是"通用 setter": schema 渲染的控件、迁移期旧控件、tab 切换都走这里,
  /// 不再逐键 setState + setSetting。
  void _writeSetting(String key, dynamic value) {
    final previous = _settingValues[key];
    setState(() => _settingValues[key] = value);
    _setSetting(key, value);
    _afterSettingChanged(key, previous, value);
  }

  /// 设置值变化后需要同步的**动态 UI**(标签页数量/画布模式/界面语言)。
  /// 纯展示类设置不需要任何副作用(渲染由 _settingValues 驱动)。
  void _afterSettingChanged(String key, dynamic previous, dynamic value) {
    switch (key) {
      case 'showFreeCanvas':
        _syncTabCount();
        if (value != true && _infiniteCanvas) {
          // tab 移除后只剩活页本: 模式同步回翻页(旧行为)。
          _writeSetting('infiniteCanvas', false);
        }
      case 'showBubbleTab':
        _syncTabCount();
      case 'infiniteCanvas':
        _syncTabToCanvasMode(force: true);
      case 'language':
        _applyLanguage(_language);
    }
  }

  void _setLanguage(String code) =>
      _writeSetting('language', code == 'en' ? 'en' : 'zh');

  /// 只改 UI 语言, 不写库(加载/原生菜单推送时用)。
  void _applyLanguage(String code) {
    final loc = Locale(code == 'en' ? 'en' : 'zh');
    if (appLocale.value != loc) appLocale.value = loc;
  }

  /// 文本类设置项共用已有的 TextEditingController(登录/NAS 备份); 以后新增
  /// 的文本设置按 key 惰性创建, 不在状态类里加字段。
  final Map<String, TextEditingController> _extraTextControllers = {};

  TextEditingController _settingTextController(String key) {
    switch (key) {
      case 'chatApiBase':
        return _chatApiBase;
      case 'chatUser':
        return _chatUser;
      case 'nasUrl':
        return _nasUrl;
      default:
        return _extraTextControllers.putIfAbsent(key, TextEditingController.new);
    }
  }

  /// 统一把"读失败"变成用户看得见的一条提示, 而不是悄悄显示成空列表。
  void _toastError(String message) {
    if (!mounted) return;
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(content: Text(message), duration: const Duration(seconds: 4)),
    );
  }

  void _onGridScroll() {
    _gridScrollDebounce?.cancel();
    _gridScrollDebounce = Timer(const Duration(milliseconds: 80), () {
      if (mounted) _updateVisibleRange();
    });
  }

  /// 计算当前可视的页面索引范围,只加载这些页面的缩略图。
  /// 跳到第 500 页时直接加载第 500 页附近,不再等前 499 张。
  void _updateVisibleRange() {
    if (!_gridScroll.hasClients || _filteredPages.isEmpty) return;
    final pos = _gridScroll.position;
    final viewportH = pos.viewportDimension;
    final scrollPx = pos.pixels;
    final gw = context.size?.width ?? 300;
    // 与 maxCrossAxisExtent: 300 的网格布局保持一致
    final cols = (gw / 300).ceil().clamp(1, 10);
    final cardW = gw / cols;
    final cardH = cardW / 1.2 + 30; // 卡片纵横比 + 标题区
    final rowsTotal = (_filteredPages.length / cols).ceil();
    final firstRow = (scrollPx / cardH).floor().clamp(0, rowsTotal - 1);
    final lastRow = ((scrollPx + viewportH) / cardH).ceil().clamp(0, rowsTotal);
    final lo = ((firstRow - 1) * cols).clamp(0, _filteredPages.length);
    final hi = ((lastRow + 1) * cols).clamp(0, _filteredPages.length);
    final want = <int>[];
    for (var i = lo; i < hi; i++) {
      final page = _filteredPages[i];
      if (page.thumbnail == null && !_thumbnailCache.containsKey(page.id)) {
        want.add(page.id);
      }
    }
    _requestThumbnails(want);
  }

  void _onTabChanged() {
    // 由 _syncTabToCanvasMode 程序性切换: 只同步显示/加载, 不再反向发设置
    if (_syncingTabs) return;
    final idx = _tabController.index;
    // 切 tab 同时切换画布模式:活页本=翻页模式,自由涂鸦=无限画布。
    // 本地状态必须立即更新:否则再点回原 tab 时,Flutter 以为模式没变而不发消息。
    if (idx == 1 && _infiniteCanvas) {
      _writeSetting('infiniteCanvas', false);
    } else if (idx == 2 && _showFreeCanvas && !_infiniteCanvas) {
      _writeSetting('infiniteCanvas', true);
    }
    _afterTabActivated(idx);
  }

  /// tab 被选中后的内容加载(手动点击与程序性同步共用)。
  void _afterTabActivated(int idx) {
    if (idx == 1 && _pages.isEmpty && !_pagesLoading) {
      _loadPages();
    }
    if (idx == 2 && _showFreeCanvas && !_canvasEditing) {
      _loadCanvasOverview();
    }
    // 气泡消息 tab: 位置在自由涂鸦之后(自由涂鸦隐藏时占 index 2)
    final bubbleIdx = 2 + (_showFreeCanvas ? 1 : 0);
    if (_showBubbleTab && idx == bubbleIdx) {
      _loadBubbles();
    } else {
      _stopReplay(); // 离开气泡 tab = 停止重播(未在播时是无害空操作)
    }
  }

  /// 当前画布模式对应的 tab 下标: 自由涂鸦 = 2(存在时), 否则活页本 = 1。
  int _desiredModeTabIndex() {
    if (_showFreeCanvas && _infiniteCanvas) return 2;
    return 1;
  }

  /// 让 TabBar 选中项跟随画布模式(玻璃 → 面板方向的同步)。
  ///
  /// 画布模式可以从外部改(快捷键切换自由涂鸦、覆盖层推设置), 面板不能只更新
  /// 本地字段而不动 TabBar —— 否则会出现"玻璃在自由涂鸦、面板停在活页本"的
  /// 错位, 用户再点活页本就会把自由涂鸦关掉。设置/气泡 tab 不是画布模式选择,
  /// 非 force 时尽量不把用户从那里拽走。
  void _syncTabToCanvasMode({bool force = false}) {
    if (!mounted) return;
    final target = _desiredModeTabIndex();
    if (target >= _tabController.length) return;
    if (_tabController.index == target) return;
    final freeIdx = _showFreeCanvas ? 2 : -1;
    final onCanvasTab = _tabController.index == 1 || _tabController.index == freeIdx;
    if (!force && !onCanvasTab) return;
    _syncingTabs = true;
    _tabController.index = target;
    _syncingTabs = false;
    // 程序性同步也要加载对应内容(否则自由涂鸦 tab 打开后总览一直空/活页本不加载)
    _afterTabActivated(target);
  }

  /// tab 数量跟随「自由涂鸦」开关(开 = 3 个;默认关 = 2 个:设置+活页本)。
  /// 长度变化必须重建 controller,否则 TabBar/TabBarView 断言崩溃。
  void _syncTabCount() {
    final want = 2 + (_showFreeCanvas ? 1 : 0) + (_showBubbleTab ? 1 : 0);
    if (_tabController.length == want) return;
    final old = _tabController;
    // 保留用户当前所在 tab(数量变化时夹到合法范围), 不要无条件跳回设置页
    final keepIndex = old.index.clamp(0, want - 1);
    _tabController = TabController(length: want, vsync: this, initialIndex: keepIndex);
    _tabController.addListener(() {
      if (!mounted) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _updateVisibleRange();
      });
    });
    _tabController.addListener(_onTabChanged);
    old.dispose();
  }

  /// 设置页装配: 有 schema 时按后端 uiConfig 渲染(迁移期未迁移分区走旧
  /// builder 兜底); uiConfig 拿不到(管道未连/测试降级)时整页回退旧手写表单。
  Widget _buildSettingsTab() {
    if (uiConfig.settingsSections.isEmpty) return _legacySettingsScroll();
    return _buildSchemaSettingsTab();
  }

  /// 旧手写设置页(迁移期兜底; uiConfig 不可用时保证面板仍可操作)。
  Widget _legacySettingsScroll() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        key: _columnKey,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: _legacySettingsChildren(),
      ),
    );
  }

  /// 未迁移分区(第三步清空): 复用旧手写内容, 只由 schema 决定分区位置/标题。
  Widget? _legacySectionFor(String id) {
    final l = AppLocalizations.of(context)!;
    switch (id) {
      case 'notebookLook':
        return _buildSection(l.notebookLook, _legacyNotebookLookContent());
      case 'recordExport':
        return _buildSection(l.recordExport, _buildRecordExportSection());
      case 'dataBackup':
        return _buildSection(l.dataBackup, _buildBackupButtons());
      case 'doodleIdentity':
        return _buildSection(l.doodleIdentity, _buildChatAuthSection());
      case 'shortcuts':
        return _buildSection(_shortcutSectionTitle(), _buildShortcutEditor());
      case 'about':
        return _buildSection(l.about, _buildAboutSection());
      default:
        return null;
    }
  }

  /// 旧「活页本外观」内容(第三步由 schema 控件替换)。
  Widget _legacyNotebookLookContent() {
    return Column(children: [
Row(children: [
        Text(AppLocalizations.of(context)!.style, style: TextStyle(fontSize: 13)),
        const Spacer(),
        SegmentedButton<bool>(
          segments: [
            ButtonSegment(value: true, label: Text(AppLocalizations.of(context)!.skeuoGlass)),
            ButtonSegment(value: false, label: Text(AppLocalizations.of(context)!.notePaper)),
          ],
          selected: {_notebookGlass},
          showSelectedIcon: false,
          onSelectionChanged: (s2) => _writeSetting('notebookStyle', s2.first),
        ),
      ]),
      if (uiConfig.features['pageFlipEffect'] == true) ...[
        const Divider(height: 20),
        Row(children: [
          Text(AppLocalizations.of(context)!.pageFlipEffect,
              style: TextStyle(fontSize: 13, color: _inkFaint)),
          const Spacer(),
          SegmentedButton<int>(
            showSelectedIcon: false,
            style: const ButtonStyle(
              visualDensity: VisualDensity(horizontal: -2, vertical: -2),
            ),
            segments: [
              ButtonSegment(value: 0, label: Text(AppLocalizations.of(context)!.systemAnimation)),
              ButtonSegment(value: 1, label: Text(AppLocalizations.of(context)!.timeTunnel)),
            ],
            selected: {_flipEffect},
            onSelectionChanged: (sel) => _writeSetting('flipEffect', sel.first),
          ),
        ]),
      ],
      const Divider(height: 20),
      _tile(SwitchListTile(
        title: Text(AppLocalizations.of(context)!.show10NearbyPages, style: TextStyle(fontSize: 14)),
        subtitle: Text(AppLocalizations.of(context)!.verticalMinimapOnThe, style: TextStyle(fontSize: 12)),
        value: _minimapEnabled,
        dense: true,
        contentPadding: EdgeInsets.zero,
        onChanged: (v) => _writeSetting('minimap', v),
      )),
      const Divider(height: 20),
      _tile(SwitchListTile(
        title: Text(AppLocalizations.of(context)!.truePassThroughExperimental, style: TextStyle(fontSize: 14)),
        subtitle: Text(AppLocalizations.of(context)!.cardsRevealTheDesktop, style: TextStyle(fontSize: 12)),
        value: _panelTransparent && _notebookGlass,
        dense: true,
        contentPadding: EdgeInsets.zero,
        onChanged: _notebookGlass ? (v) => _writeSetting('panelTransparent', v) : null,
      )),
    ]);
  }

  /// 旧手写设置页的 children(迁移期兜底; 第三步删除)。
  List<Widget> _legacySettingsChildren() {
    return [
// ── 书写: 最常用的放最前 ──
      _buildSection(AppLocalizations.of(context)!.pen, _buildPenSection()),
      const SizedBox(height: 16),
      _buildMoreSettingsToggle(),
      if (_showAdvanced) ...[
        const SizedBox(height: 12),
        _buildSection(AppLocalizations.of(context)!.canvasLook, _buildAppearanceSection()),
        const SizedBox(height: 16),
        // ── 活页本: 外观 / 翻页动效 / 缩略图 / 真穿透 ──
        _buildSection(AppLocalizations.of(context)!.notebookLook, _legacyNotebookLookContent()),
        const SizedBox(height: 16),
        // ── 录制与导出 ──
        _buildSection(AppLocalizations.of(context)!.recordExport, _buildRecordExportSection()),
        const SizedBox(height: 16),
        _buildSection(AppLocalizations.of(context)!.dataBackup, _buildBackupButtons()),
        const SizedBox(height: 16),
        if (_vBool('chatIntegration'))
          _buildSection(AppLocalizations.of(context)!.sharedCanvas, _tile(SwitchListTile(
            title: Text(AppLocalizations.of(context)!.streamDoodlesLive, style: TextStyle(fontSize: 14)),
            subtitle: Text(AppLocalizations.of(context)!.pushedToThePeer,
                style: TextStyle(fontSize: 12)),
            value: _vBool('shareCanvas'),
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) => _writeSetting('shareCanvas', v),
          ))),
        if (_vBool('chatIntegration')) const SizedBox(height: 16),
        _buildSection(AppLocalizations.of(context)!.doodleIdentity, _buildChatAuthSection()),
        const SizedBox(height: 16),
        // ── 系统与辅助 ──
        _buildSection(AppLocalizations.of(context)!.systemAndAssist, _buildSystemSection()),
        const SizedBox(height: 16),
        // ── 少用的放最后: 语言 / 快捷键 ──
        _buildSection(
          AppLocalizations.of(context)!.language,
          SegmentedButton<String>(
            segments: [
              ButtonSegment(
                  value: 'zh',
                  label: Text(AppLocalizations.of(context)!.chinese)),
              ButtonSegment(
                  value: 'en',
                  label: Text(AppLocalizations.of(context)!.english)),
            ],
            selected: {_language},
            showSelectedIcon: false,
            onSelectionChanged: (sel) => _setLanguage(sel.first),
          ),
        ),
        const SizedBox(height: 16),
        _buildSection(
          _shortcutSectionTitle(),
          _buildShortcutEditor(),
        ),
      ],
      const SizedBox(height: 16),
      // 一般软件的「检查更新」都在最底部: 关于区(版本 + 检查更新)放最后
      _buildSection(AppLocalizations.of(context)!.about, _buildAboutSection()),
    ];
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Column(
        children: [
          // 顶栏:tab(原 AppBar 已按需求去掉)
          Container(
            color: _paperBg,
            padding: const EdgeInsets.only(top: 6),
            child: Row(
              children: [
                Expanded(
                  child: TabBar(
                    controller: _tabController,
                    tabs: [
                  Tab(
                    text: AppLocalizations.of(context)!.settings,
                  ),
                  Tab(
                    child: _modeTabLabel(AppLocalizations.of(context)!.notebook, !_infiniteCanvas),
                  ),
                  if (_showFreeCanvas)
                    Tab(
                      child: _modeTabLabel(AppLocalizations.of(context)!.freeDoodle, _infiniteCanvas),
                    ),
                  if (_showBubbleTab)
                    Tab(text: AppLocalizations.of(context)!.bubbleTab),
                  ],
                ),
                ),
              ],
            ),
          ),
          Expanded(
            child: TabBarView(
              controller: _tabController,
              children: [
            // ── Settings tab: schema(D11b)──
            _tabBackground('assets/tab_bg_settings.jpg', _buildSettingsTab()),
            // ── Content(活页本) tab ──
            // 真穿透: 不画底图, 卡片窟窿透出面板后的桌面
            (_panelTransparent && _notebookGlass
                ? _buildContentTab()
                : _tabBackground('assets/tab_bg_pages.jpg', _buildContentTab())),
            if (_showFreeCanvas)
              _tabBackground('assets/tab_bg_infinite.jpg', _buildCanvasTab()),
            if (_showBubbleTab) _buildBubbleTab(),
          ],
        ),
          ),
        ],
      ),
    );
  }

  Widget _buildContentTab() {
    return Column(
      children: [
        // 内容 tab 只留内容相关 UI(网格工具条); 设置区全部在「设置」tab。
        if (!_pagesLoading && _openNotebook != null && _filteredPages.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: _buildGridToolbar(),
          ),
        // Page grid: 两级 —— 未开本子 = 笔记本卡片; 开了 = 该本子的页
        Expanded(
          child: _pagesLoading
              ? const Center(child: CircularProgressIndicator())
              : _detailPageId != null
                  ? _buildPageDetail()
                  : _openNotebook == null
                      ? _buildNotebookGrid()
                      : _buildNotebookPages(),
        ),
      ],
    );
  }

  bool _backupBusy = false;
  bool _nasBusy = false; // NAS 备份进行中
  final _nasUrl = TextEditingController(); // NAS 上 axum 服务地址
  bool _pdfExporting = false;
}
