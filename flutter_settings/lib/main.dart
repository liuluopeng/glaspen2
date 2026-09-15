import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
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
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768}) async => {};

  @override
  Future<Map<dynamic, dynamic>> canvasHome() async => {};

  @override
  Future<Map<dynamic, dynamic>> canvasCenter() async => {};

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

// ── App ──

class GlaspenSettingsApp extends StatelessWidget {
  const GlaspenSettingsApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Glaspen2 Settings',
      debugShowCheckedModeBanner: false,
      // macOS runs at native 1x (no display scaling), so the whole UI looks
      // small vs Windows — scale the entire UI up to match. Windows keeps 1.0
      // (the OS display scaling handles it).
      builder: (context, child) {
        final s = Platform.isMacOS ? 1.4 : 1.0;
        if (s == 1.0) return child!;
        return Transform.scale(
          scale: s,
          child: FractionallySizedBox(
            widthFactor: 1 / s,
            heightFactor: 1 / s,
            child: child,
          ),
        );
      },
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.light,
        colorSchemeSeed: Colors.blueGrey,
        fontFamily: 'LXGWWenKaiMono',
      ),
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
    _tabController =
        TabController(length: Platform.isMacOS ? 3 : 2, vsync: this);
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
    if (_tabController.index == 1 && _pages.isEmpty && !_pagesLoading) {
      _loadPages();
    }
    if (_tabController.index == 2) {
      _loadCanvasOverview();
    }
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
        _canvasPng = payload['png'] as Uint8List?;
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
          : _bridge.canvasCenter());
      if (!mounted) return;
      setState(() {
        _canvasPng = payload['png'] as Uint8List?;
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

  /// 画布总览 tab(仅 macOS;展示当前激活画布与视口位置)
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
      appBar: AppBar(
        title: const Text('Glaspen2 Settings'),
        centerTitle: true,
        actions: [
          if (!_connected)
            const Padding(
              padding: EdgeInsets.only(right: 12),
              child: Icon(Icons.cloud_off, color: Colors.red, size: 20),
            ),
        ],
        bottom: TabBar(
          controller: _tabController,
          tabs: [
            const Tab(text: '设置'),
            const Tab(text: '内容'),
            if (Platform.isMacOS) const Tab(text: '画布'),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabController,
        children: [
            // ── Settings tab ──
            SingleChildScrollView(
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
                  _buildSection('Color', _buildColorGrid()),
                  const SizedBox(height: 16),
                  _buildSection('Width', _buildWidthRow()),
                  const SizedBox(height: 16),
                  _buildSection('Actions', _buildActionButtons()),
                  const SizedBox(height: 16),
                  _buildSection('GIF 动画 (⌘⌃R 录制)', _buildGifSettings()),
                  const SizedBox(height: 16),
                  _buildSection('Options', _buildToggles()),
                  const SizedBox(height: 16),
                  _buildSection('Export', _buildExportButtons()),
                ],
              ),
            ),
            // ── Content tab ──
            _buildContentTab(),
            if (Platform.isMacOS) _buildCanvasTab(),
          ],
        ),
      );
  }

  Widget _buildContentTab() {
    return Column(
      children: [
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title,
            style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 15)),
        const SizedBox(height: 8),
        child,
      ],
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

  Widget _buildActionButtons() {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        ElevatedButton.icon(
          onPressed: () {
            _setSetting('undo', true);
          },
          icon: const Icon(Icons.undo, size: 16),
          label: const Text('撤销上一笔', style: TextStyle(fontSize: 15)),
          style: ElevatedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
        ElevatedButton.icon(
          onPressed: () {
            _setSetting('export_animated_gif', true);
          },
          icon: const Icon(Icons.gif, size: 16),
          label: const Text('导出动画 GIF', style: TextStyle(fontSize: 15)),
          style: ElevatedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
      ],
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

  Widget _buildToggles() {
    return Column(
      children: [
        SwitchListTile(
          title: const Text('压力监控', style: TextStyle(fontSize: 15)),
          value: _pressureMonitor,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _pressureMonitor = v);
            _setSetting('pressureMonitor', v);
          },
        ),
        SwitchListTile(
          title: const Text('显示网格 (40px)', style: TextStyle(fontSize: 15)),
          subtitle: const Text('涂鸦时辅助对齐', style: TextStyle(fontSize: 12)),
          value: _showGrid,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _showGrid = v);
            _setSetting('grid', v);
          },
        ),
        SwitchListTile(
          title: const Text('网格跟随涂鸦', style: TextStyle(fontSize: 15)),
          subtitle: const Text('开启后网格随涂鸦一起受 ⌘⌃X 控制', style: TextStyle(fontSize: 12)),
          value: _gridFollowStrokes,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _gridFollowStrokes = v);
            _setSetting('gridFollowStrokes', v);
          },
        ),
        SwitchListTile(
          title: const Text('笔迹描边', style: TextStyle(fontSize: 15)),
          subtitle: const Text('按笔色亮度自动加反色描边 · 渲染设置,不持久化', style: TextStyle(fontSize: 12)),
          value: _outlineEnabled,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _outlineEnabled = v);
            _setSetting('outline', v);
          },
        ),
        if (Platform.isMacOS)
          SwitchListTile(
            title: const Text('无限画布', style: TextStyle(fontSize: 15)),
            subtitle: const Text('⌥⇧滚轮缩放(上限100%) · ⌥⌘方向键平移 · 菜单里也有同一开关', style: TextStyle(fontSize: 12)),
            value: _infiniteCanvas,
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) {
              setState(() => _infiniteCanvas = v);
              _setSetting('infiniteCanvas', v);
            },
          ),
        if (Platform.isMacOS)
          Padding(
            padding: const EdgeInsets.only(top: 4, bottom: 8),
            child: Row(
              children: [
                const Text('网格大小', style: TextStyle(fontSize: 15)),
                const Spacer(),
                SegmentedButton<int>(
                  showSelectedIcon: false,
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
      ],
    );
  }

  Widget _buildExportButtons() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          '将当前笔迹按笔顺生成为动画 GIF，自动复制到剪贴板并保存到桌面。',
          style: TextStyle(fontSize: 13, color: Colors.grey),
        ),
        const SizedBox(height: 8),
        FilledButton.icon(
          icon: _gifExporting
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: Colors.white,
                  ),
                )
              : const Icon(Icons.animation, size: 18),
          label: Text(_gifExporting ? '生成中…' : '导出动画 GIF'),
          onPressed: _gifExporting ? null : _exportAnimatedGif,
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

  bool _gifExporting = false;

  Future<void> _exportAnimatedGif() async {
    setState(() => _gifExporting = true);
    try {
      if (Platform.isMacOS) {
        final ok = await _channel.invokeMethod<bool>('exportAnimatedGif') == true;
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(ok
                  ? '动画 GIF 已保存并复制到剪贴板'
                  : '没有笔迹或导出失败'),
              duration: const Duration(seconds: 2),
            ),
          );
        }
      } else {
        // Windows:通过管道触发导出,结果以屏幕通知提示
        _setSetting('export_animated_gif', true);
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('导出失败: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _gifExporting = false);
    }
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
