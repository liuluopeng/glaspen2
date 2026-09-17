import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

void main() => runApp(const GlaspenSettingsApp());

// ── Platform-specific communication ──

const _channel = MethodChannel('com.glaspen/settings');
const _pipeName = r'\\.\pipe\glaspen2_settings';

/// Abstract interface for settings communication.
abstract class _SettingsBridge {
  Future<Map<dynamic, dynamic>> getSettings();
  Future<void> setSetting(String key, dynamic value);
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback);
  /// 连接建立后回调(Windows 管道异步连接;用于连接后重新拉取设置)
  void Function()? onConnected;
  /// Content tab: page list JSON
  Future<String> listPages();
  /// Content tab: page thumbnail PNG bytes (null if none)
  Future<Uint8List?> getPageThumbnail(int screenId, int w, int h, int maxSize);
  /// 跳转到指定页面并恢复笔迹(继续绘画)
  Future<void> navigateToPage(int screenId);
  /// 触发一个快捷键动作(与 Ctrl+Alt+<key> 等价)
  Future<void> triggerHotkey(String key);
  /// 无限画布总览:返回 {png: Uint8List, rect: [x,y,w,h]};空画布返回 {}
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768});
  /// 镜头回到原点 + 100%,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasHome();
  /// 镜头居中到内容包围盒,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasCenter();
  /// 手动新建无限画布:清空内容 + 镜头回原点,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasNew();
  void dispose();
}

/// macOS: uses Flutter MethodChannel (embedded in ObjC host).
class _MethodChannelBridge extends _SettingsBridge {
  final void Function(Map<dynamic, dynamic>)? _onChanged;

  _MethodChannelBridge(this._onChanged) {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'onSettingsChanged' && _onChanged != null) {
        _onChanged!(call.arguments as Map<dynamic, dynamic>);
      }
    });
  }

  @override
  Future<Map<dynamic, dynamic>> getSettings() async {
    return await _channel.invokeMethod('getSettings');
  }

  @override
  Future<void> setSetting(String key, dynamic value) async {
    await _channel.invokeMethod('setSetting', {'key': key, 'value': value});
  }

  @override
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback) {
    // Handled in constructor via setMethodCallHandler
  }

  @override
  Future<String> listPages() async {
    return await _channel.invokeMethod<String>('listPages') ?? '[]';
  }

  @override
  Future<Uint8List?> getPageThumbnail(int screenId, int w, int h, int maxSize) async {
    return await _channel.invokeMethod<Uint8List>('getPageThumbnail', {
      'screenId': screenId, 'w': w, 'h': h, 'maxSize': maxSize,
    });
  }

  @override
  Future<void> navigateToPage(int screenId) async {
    await _channel.invokeMethod('navigateToPage', {'screenId': screenId});
  }

  @override
  Future<void> triggerHotkey(String key) async {
    await _channel.invokeMethod('hotkey', {'key': key});
  }

  @override
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768}) async {
    return await _channel.invokeMethod('canvasOverview', {'w': w, 'h': h}) ?? {};
  }

  @override
  Future<Map<dynamic, dynamic>> canvasHome() async {
    return await _channel.invokeMethod('canvasOverview', {'home': true}) ?? {};
  }

  @override
  Future<Map<dynamic, dynamic>> canvasCenter() async {
    return await _channel.invokeMethod('canvasOverview', {'center': true}) ?? {};
  }

  @override
  Future<Map<dynamic, dynamic>> canvasNew() async {
    return await _channel.invokeMethod('canvasNew') ?? {};
  }

  @override
  void dispose() {}
}

// FFI types
typedef _CreateFileWNative = IntPtr Function(
    Pointer<Utf16>, Uint32, Uint32, Pointer<Void>, Uint32, Uint32, IntPtr);
typedef _CreateFileWDart = int Function(
    Pointer<Utf16>, int, int, Pointer<Void>, int, int, int);
typedef _ReadWriteNative = Uint8 Function(
    IntPtr, Pointer<Uint8>, Uint32, Pointer<Uint32>, Pointer<Uint8>);
typedef _ReadWriteDart = int Function(
    int, Pointer<Uint8>, int, Pointer<Uint32>, Pointer<Uint8>);
typedef _CloseHandleNative = Uint8 Function(IntPtr);
typedef _CloseHandleDart = int Function(int);
typedef _PeekNamedPipeNative = Uint8 Function(
    IntPtr, Pointer<Uint8>, Uint32, Pointer<Uint32>, Pointer<Uint32>, Pointer<Uint32>);
typedef _PeekNamedPipeDart = int Function(
    int, Pointer<Uint8>, int, Pointer<Uint32>, Pointer<Uint32>, Pointer<Uint32>);

/// Windows: uses Named Pipe for IPC with the main overlay process.
/// Opens pipe with GENERIC_READ|GENERIC_WRITE via CreateFileW.
/// Uses ReadFile/WriteFile directly for I/O.
class _NamedPipeBridge extends _SettingsBridge {
  int _handle = -1; // Windows HANDLE
  final _buffer = <int>[];
  void Function(Map<dynamic, dynamic>)? _onChanged;
  Completer<Map<dynamic, dynamic>>? _settingsCompleter;
  /// 并发请求表:reqId -> completer(缩略图等多请求并发)
  final _pendingReqs = <int, Completer<Map<dynamic, dynamic>>>{};
  int _reqSeq = 0;
  bool _connected = false;
  Timer? _reconnectTimer;
  Timer? _readTimer;

  late final DynamicLibrary _kernel32;
  late final _ReadWriteDart _readFile;
  late final _ReadWriteDart _writeFile;
  late final _CloseHandleDart _closeHandle;
  late final _PeekNamedPipeDart _peekNamedPipe;

  _NamedPipeBridge() {
    _kernel32 = DynamicLibrary.open('kernel32.dll');
    _readFile = _kernel32
        .lookupFunction<_ReadWriteNative, _ReadWriteDart>('ReadFile');
    _writeFile = _kernel32
        .lookupFunction<_ReadWriteNative, _ReadWriteDart>('WriteFile');
    _closeHandle = _kernel32
        .lookupFunction<_CloseHandleNative, _CloseHandleDart>('CloseHandle');
    _peekNamedPipe = _kernel32
        .lookupFunction<_PeekNamedPipeNative, _PeekNamedPipeDart>('PeekNamedPipe');
    _connect();
  }

  void _connect() {
    try {
      final createFileW = _kernel32
          .lookupFunction<_CreateFileWNative, _CreateFileWDart>('CreateFileW');

      final pathPtr = _pipeName.toNativeUtf16();
      // GENERIC_READ | GENERIC_WRITE
      const access = 0x80000000 | 0x40000000;
      // OPEN_EXISTING
      const disposition = 3;
      // FILE_FLAG_OVERLAPPED for async reads
      const flags = 0x40000000;

      final h = createFileW(pathPtr, access, 0, nullptr, disposition, flags, 0);
      calloc.free(pathPtr);

      if (h == -1) {
        debugPrint('[Settings] CreateFileW failed — retrying in 2s');
        _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
        return;
      }

      _handle = h;
      _connected = true;
      debugPrint('[Settings] Connected to pipe $_pipeName (handle=$_handle)');
      _startReading();
      // 连接成功后重新拉取设置(启动时 initState 的 getSettings 会因未连接返回空)
      onConnected?.call();
    } catch (e) {
      debugPrint('[Settings] Pipe connect failed: $e — retrying in 2s');
      _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
    }
  }

  void _startReading() {
    // Poll for data every 16ms (~60fps)
    _readTimer = Timer.periodic(const Duration(milliseconds: 16), (_) {
      if (!_connected) return;
      _tryRead();
    });
  }

  void _tryRead() {
    if (!_connected || _handle == -1) return;

    // Use PeekNamedPipe to check available bytes (non-blocking)
    final totalAvail = calloc<Uint32>();
    final ok = _peekNamedPipe(_handle, nullptr, 0, nullptr, totalAvail, nullptr);
    final avail = totalAvail.value;
    calloc.free(totalAvail);

    if (ok == 0) {
      // Pipe broken
      _connected = false;
      _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
      return;
    }

    if (avail == 0) return; // No data yet

    // Read available data
    final toRead = avail > 1024 ? 1024 : avail;
    final buf = calloc<Uint8>(toRead);
    final bytesRead = calloc<Uint32>();
    final success = _readFile(_handle, buf, toRead, bytesRead, nullptr);
    final count = bytesRead.value;
    calloc.free(bytesRead);

    if (success != 0 && count > 0) {
      for (int i = 0; i < count; i++) {
        final byte = buf[i];
        if (byte == 10) {
          if (_buffer.isNotEmpty) {
            final line = utf8.decode(_buffer);
            _buffer.clear();
            _handleMessage(line);
          }
        } else {
          _buffer.add(byte);
        }
      }
    }

    calloc.free(buf);
  }

  void _handleMessage(String line) {
    try {
      final msg = jsonDecode(line) as Map<String, dynamic>;
      final type = msg['type'] as String?;
      if (type == 'onSettingsChanged' && _onChanged != null) {
        _onChanged!(msg['data'] as Map<dynamic, dynamic>);
      } else if (type == 'getSettings_response' && _settingsCompleter != null) {
        _settingsCompleter!.complete(msg['data'] as Map<dynamic, dynamic>);
        _settingsCompleter = null;
      } else if (type != null && type.endsWith('_response')) {
        // 并发请求按 reqId 匹配 completer
        final id = (msg['reqId'] as num?)?.toInt() ?? 0;
        final c = _pendingReqs.remove(id);
        if (c != null) {
          final d = msg['data'];
          if (d is Map<dynamic, dynamic>) {
            c.complete(d);
          } else if (d is List<dynamic>) {
            c.complete({'data': d});
          } else {
            c.complete({});
          }
        } else {
        }
      }
    } catch (e) {
      debugPrint('[Settings] Parse error: $e');
    }
  }

  /// 通用管道请求:发送请求并等待对应 *_response 消息(支持并发,按 reqId 匹配)
  Future<Map<dynamic, dynamic>> _request(String type, Map<String, dynamic>? params) async {
    if (!_connected) {
      return {};
    }
    final id = ++_reqSeq;
    final c = Completer<Map<dynamic, dynamic>>();
    _pendingReqs[id] = c;
    final msg = <String, dynamic>{'type': type, 'reqId': id, ...?params};
    _writeData(jsonEncode(msg) + '\n');
    try {
      final r = await c.future.timeout(
        const Duration(seconds: 15),
        onTimeout: () {
          _pendingReqs.remove(id);
          return <dynamic, dynamic>{};
        },
      );
      return r;
    } catch (_) {
      _pendingReqs.remove(id);
      return {};
    }
  }

  @override
  Future<String> listPages() async {
    final r = await _request('listPages', null);
    final data = r['data'];
    return data == null ? '[]' : jsonEncode(data);
  }

  @override
  Future<Uint8List?> getPageThumbnail(int screenId, int w, int h, int maxSize) async {
    final r = await _request('getPageThumbnail', {
      'screenId': screenId, 'w': w, 'h': h, 'maxSize': maxSize,
    });
    final png = r['png'] as String?;
    if (png == null || png.isEmpty) return null;
    try {
      final bytes = base64Decode(png);
      return bytes;
    } catch (e) {
      return null;
    }
  }

  @override
  Future<void> navigateToPage(int screenId) async {
    if (!_connected) return;
    _writeData(jsonEncode({'type': 'navigateToPage', 'screenId': screenId}) + '\n');
  }

  @override
  Future<void> triggerHotkey(String key) async {
    if (!_connected) return;
    _writeData(jsonEncode({'type': 'hotkey', 'key': key}) + '\n');
  }

  @override
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768}) async =>
      _request('canvasOverview', {'w': w, 'h': h});

  @override
  Future<Map<dynamic, dynamic>> canvasHome() async =>
      _request('canvasOverview', {'home': true});

  @override
  Future<Map<dynamic, dynamic>> canvasCenter() async =>
      _request('canvasOverview', {'center': true});

  @override
  Future<Map<dynamic, dynamic>> canvasNew() async => _request('canvasNew', null);

  bool _writeData(String data) {
    if (!_connected || _handle == -1) return false;
    final bytes = utf8.encode(data);
    final buf = calloc<Uint8>(bytes.length);
    for (int i = 0; i < bytes.length; i++) {
      buf[i] = bytes[i];
    }
    final written = calloc<Uint32>();
    final success = _writeFile(_handle, buf, bytes.length, written, nullptr);
    calloc.free(buf);
    calloc.free(written);
    return success != 0;
  }

  @override
  Future<Map<dynamic, dynamic>> getSettings() async {
    if (!_connected) return {};
    try {
      _settingsCompleter = Completer<Map<dynamic, dynamic>>();
      _writeData(jsonEncode({'type': 'getSettings'}) + '\n');
      final r = await _settingsCompleter!.future.timeout(
        const Duration(seconds: 3),
        onTimeout: () {
          _settingsCompleter = null;
          return <dynamic, dynamic>{};
        },
      );
      return r;
    } catch (e) {
      debugPrint('[Settings] getSettings error: $e');
      _settingsCompleter = null;
      return {};
    }
  }

  @override
  Future<void> setSetting(String key, dynamic value) async {
    if (!_connected) return;
    try {
      _writeData(jsonEncode({'type': 'setSetting', 'key': key, 'value': value}) + '\n');
    } catch (e) {
      debugPrint('[Settings] setSetting error: $e');
    }
  }

  @override
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback) {
    _onChanged = callback;
  }

  @override
  void dispose() {
    _reconnectTimer?.cancel();
    _readTimer?.cancel();
    _connected = false;
    if (_handle != -1) {
      _closeHandle(_handle);
      _handle = -1;
    }
  }
}

/// Create the appropriate bridge for the current platform.
_SettingsBridge createBridge() {
  if (Platform.isWindows) {
    return _NamedPipeBridge();
  }
  // macOS: use MethodChannel (default)
  return _MethodChannelBridge(null);
}

// ── Data models ──

class _PageInfo {
  final int id;
  final int w;
  final int h;
  Uint8List? thumbnail;

  _PageInfo({
    required this.id,
    required this.w,
    required this.h,
  });

  factory _PageInfo.fromJson(Map<String, dynamic> json) {
    return _PageInfo(
      id: json['id'] as int,
      w: json['w'] as int,
      h: json['h'] as int,
    );
  }
}

// ── 纸墨主题 ──
const _paperBg = Color(0xFFF3EEE3);   // 暖纸底(最底层,不透明)
const _paperCard = Color(0x9EFBF8F1); // 区块卡纸面(半透明:让底层插画透出来)
const _ink = Color(0xFF2C2A26);       // 墨色文字
const _inkFaint = Color(0xFF8A857A);  // 淡墨(次要)
const _penRed = Color(0xFFC4353F);    // 笔锋红(强调色)
const _paperLine = Color(0xFFE0D9C8); // 分隔/描边

ThemeData _buildPaperTheme() {
  final scheme = ColorScheme.light(
    primary: _penRed,
    onPrimary: Colors.white,
    secondary: const Color(0xFF0070BD),
    surface: _paperCard,
    onSurface: _ink,
    surfaceContainerHighest: _paperBg,
    outline: const Color(0xFFD8D2C4),
    outlineVariant: const Color(0xFFE4DECF),
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: Colors.transparent,
    fontFamily: 'LXGWWenKaiMono',
    textTheme: ThemeData(brightness: Brightness.light)
        .textTheme
        .apply(
            bodyColor: _ink,
            displayColor: _ink,
            fontFamily: 'LXGWWenKaiMono'),
    appBarTheme: const AppBarTheme(
      backgroundColor: _paperBg,
      foregroundColor: _ink,
      elevation: 0,
      centerTitle: true,
      titleTextStyle: TextStyle(
          fontFamily: 'LXGWWenKaiMono',
          fontSize: 18,
          fontWeight: FontWeight.bold,
          color: _ink),
    ),
    tabBarTheme: const TabBarThemeData(
      labelColor: _ink,
      unselectedLabelColor: _inkFaint,
      indicatorColor: _penRed,
      dividerColor: _paperLine,
    ),
    dividerTheme: const DividerThemeData(color: _paperLine),
    switchTheme: SwitchThemeData(
      thumbColor: WidgetStateProperty.resolveWith(
          (s) => s.contains(WidgetState.selected) ? Colors.white : const Color(0xFFB9B2A2)),
      trackColor: WidgetStateProperty.resolveWith(
          (s) => s.contains(WidgetState.selected) ? _penRed : const Color(0xFFD8D2C4)),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: _ink,
        side: const BorderSide(color: Color(0xFFD8D2C4)),
        backgroundColor: _paperCard,
      ),
    ),
    // 卡片(页面缩略图等)也半透明,与模块卡纸一致
    cardTheme: CardThemeData(
      color: _paperCard,
      elevation: 0,
      shape: RoundedRectangleBorder(
        side: const BorderSide(color: _paperLine),
        borderRadius: BorderRadius.circular(10),
      ),
    ),
  );
}

/// 方格手账纸的点阵背景
class _PaperDotsPainter extends CustomPainter {
  const _PaperDotsPainter();
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = const Color(0x1A2C2A26);
    const gap = 24.0;
    for (double y = gap; y < size.height; y += gap) {
      for (double x = gap; x < size.width; x += gap) {
        canvas.drawCircle(Offset(x, y), 1.0, paint);
      }
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

// ── App ──

class GlaspenSettingsApp extends StatelessWidget {
  const GlaspenSettingsApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '玻璃涂鸦',
      debugShowCheckedModeBanner: false,
      // macOS runs at native 1x (no display scaling), so the whole UI looks
      // small vs Windows — scale the entire UI up to match. Windows keeps 1.0
      // (the OS display scaling handles it).
      builder: (context, child) {
        final s = Platform.isMacOS ? 1.4 : 1.0;
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

class _SettingsPageState extends State<SettingsPage> with SingleTickerProviderStateMixin {
  final _columnKey = GlobalKey();
  late _SettingsBridge _bridge;
  late TabController _tabController;
  int _selectedColor = 0;
  int _selectedWidth = 2;
  bool _pressureMonitor = false;
  bool _showGrid = false;
  bool _gridFollowStrokes = false;
  bool _outlineEnabled = false;
  bool _infiniteCanvas = false;
  int _gridSizeValue = 40; // 网格大小(逻辑 px)
  bool _minimapEnabled = false;
  // 画布总览 tab
  Uint8List? _canvasPng;
  List<double>? _canvasRect;
  bool _canvasLoading = false;
  bool _connected = false;
  Timer? _reloadTimer;

  // GIF quality/speed (⌘⌃R 快捷录制)
  int _gifFps = 15;
  double _gifResolution = 0.5;
  double _gifSpeed = 2.0;
  int _gifEndMode = 1; // 0=停在最后, 1=停1秒后循环, 2=立即循环

  // 10 colors, matching Rust COLOR_PRESETS / macOS g_color_presets
  // (红橙黄绿青蓝紫粉白黑). Index must match the overlay's preset order.
  // 对齐 rnote 实测色板:全部 S=100% 全饱和,鲜艳度优先(浅色场景配描边)
  static const _colorNames = ['红色', '橙色', '黄色', '绿色', '青色', '蓝色', '紫色', '粉色', '白色', '黑色'];
  static const _colorValues = [
    0xFFD6003A, 0xFFFF4D00, 0xFFFCB700, 0xFF00B16E, 0xFF6EC4F4,
    0xFF0070BD, 0xFF8A00E6, 0xFFFF0080, 0xFFFFFFFF, 0xFF000000,
  ];
  static const _widthNames = ['极细', '很细', '细', '中', '粗', '很粗', '超粗', '极粗'];

  // Content tab state
  List<_PageInfo> _pages = [];
  List<_PageInfo> _filteredPages = [];
  bool _pagesLoading = false;
  final _thumbnailCache = <int, Uint8List>{};
  final _loadingThumbnails = <int>{};

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _tabController.addListener(_onTabChanged);
    _bridge = createBridge();
    _bridge.onSettingsChanged(_onSettingsChanged);
    // Windows 管道连接成功后重新拉取设置(macOS 通道立即可用,不影响)
    _bridge.onConnected = () => _loadSettings();
    _loadSettings();
  }

  @override
  void dispose() {
    _tabController.dispose();
    _reloadTimer?.cancel();
    _bridge.dispose();
    super.dispose();
  }

  void _onTabChanged() {
    final idx = _tabController.index;
    // 切 tab 同时切换画布模式:活页本=翻页模式,自由涂鸦=无限画布。
    // 本地状态必须立即更新:否则再点回原 tab 时,Flutter 以为模式没变而不发消息。
    if (idx == 1 && _infiniteCanvas) {
      setState(() => _infiniteCanvas = false);
      _setSetting('infiniteCanvas', false);
    } else if (idx == 2 && !_infiniteCanvas) {
      setState(() => _infiniteCanvas = true);
      _setSetting('infiniteCanvas', true);
    }
    if (idx == 1 && _pages.isEmpty && !_pagesLoading) {
      _loadPages();
    }
    if (idx == 2) {
      _loadCanvasOverview();
    }
  }

  /// 模式 tab 标签:激活的模式带一颗小圆点
  Widget _modeTabLabel(String text, bool active) {
    return Row(mainAxisSize: MainAxisSize.min, children: [
      Container(
        width: 6,
        height: 6,
        margin: const EdgeInsets.only(right: 5),
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: active ? _penRed : Colors.transparent,
        ),
      ),
      Text(text),
    ]);
  }

  /// 总览载荷里的 png:macOS 通道给 Uint8List,Windows 管道给 base64 字符串
  static Uint8List? _pngBytes(dynamic v) {
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

  /// 拉取无限画布总览(当前激活画布按内容包围盒适配 + 当前视口矩形)
  Future<void> _loadCanvasOverview(
      {Map<dynamic, dynamic>? action}) async {
    if (_canvasLoading) return;
    setState(() => _canvasLoading = true);
    try {
      final payload = action ?? await _bridge.canvasOverview();
      if (!mounted) return;
      setState(() {
        _canvasPng = _pngBytes(payload['png']);
        final r = payload['rect'] as List<dynamic>?;
        _canvasRect =
            (r == null || r.length < 4) ? null : r.map((e) => (e as num).toDouble()).toList();
        _canvasLoading = false;
      });
    } catch (e) {
      debugPrint('[Canvas] overview error: $e');
      if (mounted) setState(() => _canvasLoading = false);
    }
  }

  Future<void> _canvasAction(String action) async {
    setState(() => _canvasLoading = true);
    try {
      final payload = await (action == 'home'
          ? _bridge.canvasHome()
          : action == 'new'
              ? _bridge.canvasNew()
              : _bridge.canvasCenter());
      if (!mounted) return;
      setState(() {
        _canvasPng = _pngBytes(payload['png']);
        final r = payload['rect'] as List<dynamic>?;
        _canvasRect =
            (r == null || r.length < 4) ? null : r.map((e) => (e as num).toDouble()).toList();
        _canvasLoading = false;
      });
    } catch (e) {
      debugPrint('[Canvas] $action error: $e');
      if (mounted) setState(() => _canvasLoading = false);
    }
  }

  /// 淡色章鱼插画作为 tab 的底图:铺满、很淡、不拦截点击。
  Widget _tabBackground(String asset, Widget child) {
    return Stack(
      children: [
        Positioned.fill(
          child: IgnorePointer(
            child: Opacity(
              opacity: 0.30,
              child: Image.asset(
                asset,
                fit: BoxFit.cover,
                alignment: Alignment.centerLeft,
              ),
            ),
          ),
        ),
        Positioned.fill(child: child),
      ],
    );
  }

  /// 画布总览 tab(展示当前激活画布与视口位置)
  Widget _buildCanvasTab() {
    final body = _canvasLoading
        ? const Center(child: CircularProgressIndicator())
        : (_canvasPng == null
            ? const Center(
                child: Text('空画布,先去涂鸦吧', style: TextStyle(fontSize: 14, color: Colors.grey)))
            : InteractiveViewer(
                maxScale: 8,
                child: Center(
                  child: LayoutBuilder(builder: (context, constraints) {
                    final image = Image.memory(_canvasPng!);
                    final rect = _canvasRect;
                    return Stack(children: [
                      image,
                      if (rect != null)
                        Positioned(
                          left: rect[0],
                          top: rect[1],
                          width: rect[2],
                          height: rect[3],
                          child: Container(
                            decoration: BoxDecoration(
                              border: Border.all(color: Colors.red, width: 2),
                            ),
                          ),
                        ),
                    ]);
                  }),
                ),
              ));
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(8),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton.icon(
                onPressed: _canvasLoading ? null : () => _canvasAction('home'),
                icon: const Icon(Icons.home_outlined, size: 18),
                label: const Text('回到原点', style: TextStyle(fontSize: 13)),
              ),
              OutlinedButton.icon(
                onPressed: _canvasLoading ? null : () => _canvasAction('center'),
                icon: const Icon(Icons.center_focus_strong, size: 18),
                label: const Text('居中内容', style: TextStyle(fontSize: 13)),
              ),
              if (_infiniteCanvas)
                OutlinedButton.icon(
                  onPressed: _canvasLoading ? null : () => _canvasAction('new'),
                  icon: const Icon(Icons.note_add_outlined, size: 18),
                  label: const Text('新建画布', style: TextStyle(fontSize: 13)),
                ),
              OutlinedButton.icon(
                onPressed: _canvasLoading ? null : () => _loadCanvasOverview(),
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('刷新', style: TextStyle(fontSize: 13)),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(child: body),
        const Padding(
          padding: EdgeInsets.all(6),
          child: Text('红框 = 主屏当前视口 · 涂鸦后点刷新', style: TextStyle(fontSize: 11, color: Colors.grey)),
        ),
      ],
    );
  }

  /// 服务器 JSON 里的开关是数字 0/1,统一转 bool
  static bool _b(dynamic v) => v == true || v == 1 || v == '1';

  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    if (mounted) {
      setState(() {
        _selectedColor = (s['color'] as num?)?.toInt() ?? _selectedColor;
        _selectedWidth = (s['width'] as num?)?.toInt() ?? _selectedWidth;
        _pressureMonitor = _b(s['pressureMonitor']);
        _showGrid = _b(s['grid']);
        _gridFollowStrokes = _b(s['gridFollowStrokes']);
        _outlineEnabled = _b(s['outline']);
        _infiniteCanvas = _b(s['infiniteCanvas']);
        _minimapEnabled = _b(s['minimap']);
        _gifFps = (s['gifFps'] as num?)?.toInt() ?? _gifFps;
        _gifResolution = (s['gifResolution'] as num?)?.toDouble() ?? _gifResolution;
        _gifSpeed = (s['gifSpeed'] as num?)?.toDouble() ?? _gifSpeed;
        _gifEndMode = (s['gifEndMode'] as num?)?.toInt() ?? _gifEndMode;
      });
    }
  }

  Future<void> _loadSettings() async {
    try {
      final settings = await _bridge.getSettings();
      if (mounted && settings.isNotEmpty) {
        setState(() {
          _selectedColor = (settings['color'] as num?)?.toInt() ?? 0;
          _selectedWidth = (settings['width'] as num?)?.toInt() ?? 2;
          _pressureMonitor = _b(settings['pressureMonitor']);
          _showGrid = _b(settings['grid']);
          _gridFollowStrokes = _b(settings['gridFollowStrokes']);
          _infiniteCanvas = _b(settings['infiniteCanvas']);
          _minimapEnabled = _b(settings['minimap']);
          _gifFps = (settings['gifFps'] as num?)?.toInt() ?? 15;
          _gifResolution = (settings['gifResolution'] as num?)?.toDouble() ?? 0.5;
          _gifSpeed = (settings['gifSpeed'] as num?)?.toDouble() ?? 2.0;
          _gifEndMode = (settings['gifEndMode'] as num?)?.toInt() ?? 1;
          _connected = true;
        });
      } else if (mounted) {
        // Windows 管道未就绪时 getSettings 返回空:稍后重试
        _reloadTimer = Timer(const Duration(seconds: 2), _loadSettings);
      }
    } catch (_) {
      // Fallback: use defaults if bridge not available
    }
  }

  void _setSetting(String key, dynamic value) {
    _bridge.setSetting(key, value);
  }

  // ── Content tab ──

  Future<void> _loadPages() async {
    setState(() => _pagesLoading = true);
    try {
      final json = await _bridge.listPages();
      final list = jsonDecode(json) as List<dynamic>;
      if (mounted) {
        setState(() {
          _pages = list.map((e) => _PageInfo.fromJson(e as Map<String, dynamic>)).toList();
          _filteredPages = List.from(_pages);
          _pagesLoading = false;
        });
      }
    } catch (e) {
      debugPrint('[Content] listPages error: $e');
      if (mounted) setState(() => _pagesLoading = false);
    }
  }

  Future<void> _loadThumbnail(_PageInfo page) async {
    if (_thumbnailCache.containsKey(page.id)) {
      page.thumbnail = _thumbnailCache[page.id];
      return;
    }
    try {
      final bytes = await _bridge.getPageThumbnail(page.id, page.w, page.h, 280);
      if (bytes != null && bytes.isNotEmpty && mounted) {
        _thumbnailCache[page.id] = bytes;
        page.thumbnail = bytes;
        setState(() {});
      }
    } catch (e) {
      debugPrint('[Content] thumbnail error for page ${page.id}: $e');
    } finally {
      _loadingThumbnails.remove(page.id);
    }
  }

  // ── Build ──

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Column(
        children: [
          // 顶栏:tab + 未连接指示(原 AppBar 已按需求去掉)
          Container(
            color: _paperBg,
            padding: const EdgeInsets.only(top: 6),
            child: Row(
              children: [
                Expanded(
                  child: TabBar(
                    controller: _tabController,
                    tabs: [
                  const Tab(
                    text: '设置',
                  ),
                  Tab(
                    child: _modeTabLabel('活页本', !_infiniteCanvas),
                  ),
                  Tab(
                    child: _modeTabLabel('自由涂鸦', _infiniteCanvas),
                  ),
                    ],
                  ),
                ),
                if (!_connected)
                  const Padding(
                    padding: EdgeInsets.only(right: 12),
                    child: Icon(Icons.cloud_off, color: Colors.red, size: 18),
                  ),
              ],
            ),
          ),
          Expanded(
            child: TabBarView(
              controller: _tabController,
              children: [
            // ── Settings tab ──
            _tabBackground('assets/tab_bg_settings.jpg', SingleChildScrollView(
              padding: const EdgeInsets.all(16),
              child: Column(
                key: _columnKey,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _buildSection(
                    Platform.isWindows
                        ? '快捷键 (Ctrl+Alt+…)'
                        : '快捷键 (⌘⌃…)',
                    _buildHotkeyGrid(),
                  ),
                  const SizedBox(height: 16),
                  _buildSection('笔', _buildPenSection()),
                  const SizedBox(height: 16),
                  _buildSection('Options', _buildOptionsSection()),
                ],
              ),
            )),
            // ── Content(活页本) tab ──
            _tabBackground('assets/tab_bg_pages.jpg', _buildContentTab()),
            _tabBackground('assets/tab_bg_infinite.jpg', _buildCanvasTab()),
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
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 12, 4),
          child: _buildSection('页面缩略图', _tile(SwitchListTile(
            title: const Text('显示附近 10 页', style: TextStyle(fontSize: 14)),
            subtitle: const Text('屏幕右缘竖向 minimap · 仅活页本(翻页)模式', style: TextStyle(fontSize: 12)),
            value: _minimapEnabled,
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) {
              setState(() => _minimapEnabled = v);
              _setSetting('minimap', v);
            },
          ))),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
          child: _buildSection('Export', _buildExportButtons()),
        ),
        // Page grid
        Expanded(
          child: _pagesLoading
              ? const Center(child: CircularProgressIndicator())
              : _filteredPages.isEmpty
                  ? const Center(
                      child: Text('暂无页面', style: TextStyle(fontSize: 14, color: Colors.grey)),
                    )
                  : GridView.builder(
                      itemCount: _filteredPages.length,
                      padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                        crossAxisCount: 2,
                        mainAxisSpacing: 8,
                        crossAxisSpacing: 8,
                        childAspectRatio: 1.0,
                      ),
                      itemBuilder: (context, i) {
                        final page = _filteredPages[i];
                        return _buildPageCard(page);
                      },
                    ),
        ),
      ],
    );
  }

  Widget _buildPageCard(_PageInfo page) {
    if (page.thumbnail == null && _thumbnailCache.containsKey(page.id)) {
      page.thumbnail = _thumbnailCache[page.id];
    }
    if (page.thumbnail == null && page.w > 0 && page.h > 0
        && !_loadingThumbnails.contains(page.id)) {
      _loadingThumbnails.add(page.id);
      WidgetsBinding.instance.addPostFrameCallback((_) => _loadThumbnail(page));
    }

    return Card(
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () {
          _bridge.navigateToPage(page.id);
        },
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Thumbnail
            AspectRatio(
              aspectRatio: 16 / 9,
              child: page.thumbnail != null
                  ? Image.memory(page.thumbnail!, fit: BoxFit.cover)
                  : Container(
                      color: Colors.grey.shade200,
                      child: const Icon(Icons.image_outlined, color: Colors.grey),
                    ),
            ),
            // Page info
            Padding(
              padding: const EdgeInsets.fromLTRB(8, 6, 4, 6),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('页面 ${page.id}',
                            style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.delete_outline, size: 16),
                    color: Colors.red.shade300,
                    tooltip: '删除此页面',
                    onPressed: () => _confirmDeletePage(page),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _confirmDeletePage(_PageInfo page) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('删除页面'),
        content: Text('确定删除页面 ${page.id} 及其所有笔迹吗？'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('取消')),
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              _deletePage(page);
            },
            style: TextButton.styleFrom(foregroundColor: Colors.red),
            child: const Text('删除'),
          ),
        ],
      ),
    );
  }

  Future<void> _deletePage(_PageInfo page) async {
    try {
      final ok = await _channel.invokeMethod<int>('deletePage', {'screenId': page.id}) == 1;
      if (mounted) {
        if (ok) {
          _thumbnailCache.remove(page.id);
          _pages.removeWhere((p) => p.id == page.id);
          _filteredPages.removeWhere((p) => p.id == page.id);
          setState(() {});
        } else {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('删除失败'), duration: Duration(seconds: 2)),
          );
        }
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('删除失败: $e')),
        );
      }
    }
  }

  Widget _buildSection(String title, Widget child) {
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 14),
      decoration: BoxDecoration(
        color: _paperCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: _paperLine),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(children: [
            Container(width: 3, height: 14, color: _penRed,
                margin: const EdgeInsets.only(right: 8)),
            Text(title,
                style: const TextStyle(
                    fontWeight: FontWeight.bold, fontSize: 15, color: _ink)),
          ]),
          const SizedBox(height: 10),
          child,
        ],
      ),
    );
  }

  Widget _buildColorGrid() {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: List.generate(_colorNames.length, (i) {
        final selected = i == _selectedColor;
        final isWhite = i == _colorNames.length - 1;
        return SizedBox(
          width: 60,
          height: 30,
          child: OutlinedButton(
            onPressed: () {
              setState(() => _selectedColor = i);
              _setSetting('color', i);
            },
            style: OutlinedButton.styleFrom(
              backgroundColor: Color(_colorValues[i]).withValues(alpha: 0.15),
              side: selected
                  ? const BorderSide(width: 2)
                  : BorderSide(color: Colors.grey.shade400),
              padding: EdgeInsets.zero,
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(4)),
            ),
            child: Text(
              _colorNames[i],
              style: TextStyle(
                fontSize: 13,
                color: isWhite ? Colors.black87 : Color(_colorValues[i]),
                fontWeight: selected ? FontWeight.bold : FontWeight.normal,
              ),
            ),
          ),
        );
      }),
    );
  }

  Widget _buildWidthRow() {
    return Wrap(
      spacing: 8,
      children: List.generate(_widthNames.length, (i) {
        final selected = i == _selectedWidth;
        return SizedBox(
          width: 55,
          height: 30,
          child: OutlinedButton(
            onPressed: () {
              setState(() => _selectedWidth = i);
              _setSetting('width', i);
            },
            style: OutlinedButton.styleFrom(
              backgroundColor: selected ? Colors.blueGrey.shade100 : null,
              side: selected
                  ? const BorderSide(width: 2)
                  : BorderSide(color: Colors.grey.shade400),
              padding: EdgeInsets.zero,
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(4)),
            ),
            child: Text(_widthNames[i],
                style: const TextStyle(fontSize: 13)),
          ),
        );
      }),
    );
  }

  /// GIF quality/speed settings for ⌘⌃R 快捷录制. The gray estimate refreshes
  /// as the sliders/chips change (rough per-second-of-recording file size).
  Widget _buildGifSettings() {
    const fpsOptions = [10, 15, 20, 24, 30, 50];
    const resOptions = [0.25, 0.5, 0.75, 1.0];
    const speedOptions = [0.5, 1.0, 2.0, 3.0, 4.0, 8.0, 10.0, 15.0, 20.0];

    Widget chips<T>(
        String label, List<T> options, T current, String Function(T) fmt,
        void Function(T) onPick) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 60,
            child: Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text(label, style: const TextStyle(fontSize: 14)),
            ),
          ),
          Expanded(
            child: Wrap(
              spacing: 6,
              runSpacing: 4,
              children: options.map((v) {
                final sel = v == current;
                return ChoiceChip(
                  label: Text(fmt(v)),
                  selected: sel,
                  onSelected: (_) {
                    setState(() => onPick(v));
                  },
                  showCheckmark: false,
                  labelStyle: TextStyle(
                    fontSize: 13,
                    color: sel ? Colors.white : Colors.black87,
                  ),
                  selectedColor: Colors.blueGrey.shade600,
                  visualDensity: VisualDensity.compact,
                );
              }).toList(),
            ),
          ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        chips<int>('帧率', fpsOptions, _gifFps, (v) => '$v fps', (v) {
          _gifFps = v;
          _setSetting('gifFps', v);
        }),
        const SizedBox(height: 8),
        chips<double>('分辨率', resOptions, _gifResolution, (v) => '${(v * 100).round()}%',
            (v) {
          _gifResolution = v;
          _setSetting('gifResolution', v);
        }),
        const SizedBox(height: 8),
        chips<double>('速度', speedOptions, _gifSpeed, (v) => '${v}x', (v) {
          _gifSpeed = v;
          _setSetting('gifSpeed', v);
        }),
        const SizedBox(height: 8),
        chips<int>('结束时', const [0, 1, 2], _gifEndMode,
            (v) => v == 0
                ? '停在最后'
                : v == 1
                    ? '停1秒后循环'
                    : '立即循环', (v) {
          _gifEndMode = v;
          _setSetting('gifEndMode', v);
        }),
        const SizedBox(height: 10),
        Text('预估大小 ${_estimateGifSize()}',
            style: TextStyle(fontSize: 13, color: Colors.grey.shade600)),
      ],
    );
  }

  /// Rough estimate of the GIF file size per second of recording, driven by
  /// frame rate, resolution and playback speed. Uses a reference doodle
  /// window (~720×450 logical points) and a palette-GIF compression factor.
  /// It is an approximation, not a measured result.
  String _estimateGifSize() {
    if (_gifFps <= 0 || _gifResolution <= 0 || _gifSpeed <= 0) return '—';
    const refW = 720.0;
    const refH = 450.0;
    const bytesPerPixel = 0.30; // palette-GIF LZW estimate
    final w = refW * _gifResolution;
    final h = refH * _gifResolution;
    final pixels = w * h;
    // frames packed into 1s of recording after playback-speed adjustment
    final framesPerRecordingSecond = _gifFps / _gifSpeed;
    final bytesPerSec = framesPerRecordingSecond * pixels * bytesPerPixel;
    if (bytesPerSec >= 1024 * 1024) {
      return '≈ ${(bytesPerSec / (1024 * 1024)).toStringAsFixed(1)} MB/s';
    }
    return '≈ ${(bytesPerSec / 1024).round()} KB/s';
  }

  /// 快捷键按钮,按键盘排布(如:X 在 V 左侧,按钮也在 V 左侧),
  /// 长方形按钮:键名 + 汉字功能提示
  Widget _buildHotkeyGrid() {
    Widget key(String k, String label) {
      return Tooltip(
        message: label,
        child: Padding(
          padding: const EdgeInsets.only(right: 6, bottom: 6),
          child: SizedBox(
            width: 64,
            height: 48,
            child: OutlinedButton(
              onPressed: () => _bridge.triggerHotkey(k),
              style: OutlinedButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(k,
                      style: const TextStyle(
                          fontSize: 14, fontWeight: FontWeight.bold)),
                  Text(label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 9, color: Colors.grey.shade700)),
                ],
              ),
            ),
          ),
        ),
      );
    }

    Widget row(List<Widget> children, {double indent = 0}) {
      return Padding(
        padding: EdgeInsets.only(left: indent),
        child: Row(children: children),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        row([key('Q', '退出程序')]),
        row([key('G', '导出图片'), key('J', '上一页'), key('K', '下一页')], indent: 22),
        row([
          key('Z', '撤销上一笔'),
          key('X', '飘渺画布'),
          key('C', '新建画布'),
          key('V', '开关涂鸦'),
          key('B', '模糊背景'),
        ]),
      ],
    );
  }

  // ── 笔:颜色 + 粗细 ──
  Widget _buildPenSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text('颜色',
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 8),
        _buildColorGrid(),
        const SizedBox(height: 14),
        const Text('粗细',
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 8),
        _buildWidthRow(),
      ],
    );
  }

  // ── Options:GIF 动画 + 开关组 ──
  Widget _buildOptionsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(Platform.isMacOS ? 'GIF 动画 (⌘⌃R 录制)' : 'GIF 动画 (Ctrl+Alt+R 按住录制)',
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        _buildGifSettings(),
        const Divider(height: 24),
        _buildToggles(),
      ],
    );
  }

  /// 卡片(_buildSection 的 DecoratedBox)里直接放 ListTile 会触发框架断言
  /// ("background color or ink splashes may be invisible"),包一层透明 Material。
  Widget _tile(Widget child) {
    return Material(type: MaterialType.transparency, child: child);
  }

  Widget _buildToggles() {
    return Column(
      children: [
        _tile(SwitchListTile(
          title: const Text('压力监控', style: TextStyle(fontSize: 15)),
          value: _pressureMonitor,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _pressureMonitor = v);
            _setSetting('pressureMonitor', v);
          },
        )),
        _tile(SwitchListTile(
          title: const Text('显示网格', style: TextStyle(fontSize: 15)),
          subtitle: const Text('涂鸦时辅助对齐', style: TextStyle(fontSize: 12)),
          value: _showGrid,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _showGrid = v);
            _setSetting('grid', v);
          },
        )),
        if (_showGrid && Platform.isMacOS)
          Padding(
            padding: const EdgeInsets.only(left: 16, bottom: 4),
            child: Row(
              children: [
                const Text('网格大小', style: TextStyle(fontSize: 13, color: _inkFaint)),
                const Spacer(),
                SegmentedButton<int>(
                  showSelectedIcon: false,
                  style: const ButtonStyle(
                    visualDensity: VisualDensity(horizontal: -2, vertical: -2),
                  ),
                  segments: [
                    ...[20, 40, 80].map((v) => ButtonSegment(value: v, label: Text('$v'))),
                  ],
                  selected: {_gridSizeValue},
                  onSelectionChanged: (s) {
                    setState(() => _gridSizeValue = s.first);
                    _setSetting('gridSize', s.first);
                  },
                ),
              ],
            ),
          ),
        _tile(SwitchListTile(
          title: const Text('网格跟随涂鸦', style: TextStyle(fontSize: 15)),
          subtitle: const Text('开启后网格随涂鸦一起受 ⌘⌃X 控制', style: TextStyle(fontSize: 12)),
          value: _gridFollowStrokes,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _gridFollowStrokes = v);
            _setSetting('gridFollowStrokes', v);
          },
        )),
        _tile(SwitchListTile(
          title: const Text('笔迹描边', style: TextStyle(fontSize: 15)),
          subtitle: const Text('按笔色亮度自动加反色描边 · 渲染设置,不持久化', style: TextStyle(fontSize: 12)),
          value: _outlineEnabled,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _outlineEnabled = v);
            _setSetting('outline', v);
          },
        )),
        // 页面缩略图(minimap)开关已移到「活页本」tab
        // 无限画布模式由 tab 承载:活页本=翻页模式,自由涂鸦=无限画布
        // (选中 tab 的红线下方,模式 tab 上有一颗小圆点)

      ],
    );
  }

  Widget _buildExportButtons() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          '将全部页面的笔迹导出为 PDF 保存到桌面。',
          style: TextStyle(fontSize: 13, color: Colors.grey),
        ),
        const SizedBox(height: 8),
        FilledButton.icon(
          icon: _pdfExporting
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                )
              : const Icon(Icons.picture_as_pdf, size: 18),
          label: Text(_pdfExporting ? '导出中…' : '导出全部页面为 PDF'),
          onPressed: _pdfExporting ? null : _exportPdf,
        ),
      ],
    );
  }

  bool _pdfExporting = false;

  Future<void> _exportPdf() async {
    setState(() => _pdfExporting = true);
    try {
      if (Platform.isMacOS) {
        final ok = await _channel.invokeMethod<int>('exportPdf') == 1;
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(ok ? 'PDF 已保存到桌面' : '导出失败'),
              duration: const Duration(seconds: 2),
            ),
          );
        }
      } else {
        // Windows:通过管道触发导出,结果以屏幕通知提示
        _setSetting('export_pdf', true);
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('PDF 导出失败: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _pdfExporting = false);
    }
  }
}
