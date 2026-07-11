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
  Future<String?> invokeMethod(String method, Map<String, dynamic> args);
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback);
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
  Future<String?> invokeMethod(String method, Map<String, dynamic> args) async {
    return await _channel.invokeMethod<String>(method, args);
  }

  @override
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback) {
    // Handled in constructor via setMethodCallHandler
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
  int _invokeIdCounter = 0;
  final Map<int, Completer<String?>> _invokeCompleters = {};
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
      } else if (type == 'invokeMethod_response') {
        final id = msg['id'] as int? ?? 0;
        final completer = _invokeCompleters.remove(id);
        if (completer != null) {
          final result = msg['result'];
          if (result is String) {
            completer.complete(result);
          } else if (result != null) {
            // If result is already a parsed JSON value (List/Map), re-encode it
            completer.complete(jsonEncode(result));
          } else {
            completer.complete(null);
          }
        }
      }
    } catch (e) {
      debugPrint('[Settings] Parse error: $e');
    }
  }

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
      return await _settingsCompleter!.future.timeout(
        const Duration(seconds: 3),
        onTimeout: () {
          _settingsCompleter = null;
          return <dynamic, dynamic>{};
        },
      );
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
  Future<String?> invokeMethod(String method, Map<String, dynamic> args) async {
    if (!_connected) return null;
    final id = ++_invokeIdCounter;
    try {
      final completer = Completer<String?>();
      _invokeCompleters[id] = completer;
      _writeData(jsonEncode({
        'type': 'invokeMethod',
        'id': id,
        'method': method,
        'args': args,
      }) + '\n');
      return await completer.future.timeout(
        const Duration(seconds: 10),
        onTimeout: () {
          _invokeCompleters.remove(id);
          return null;
        },
      );
    } catch (e) {
      debugPrint('[Settings] invokeMethod $method error: $e');
      _invokeCompleters.remove(id);
      return null;
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
  final String? ocr;
  Uint8List? thumbnail;

  _PageInfo({
    required this.id,
    required this.w,
    required this.h,
    this.ocr,
  });

  factory _PageInfo.fromJson(Map<String, dynamic> json) {
    return _PageInfo(
      id: json['id'] as int,
      w: json['w'] as int,
      h: json['h'] as int,
      ocr: json['ocr'] as String?,
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
  bool _smooth = true;
  bool _invert = false;
  bool _pressureMonitor = false;
  bool _showGrid = false;
  bool _connected = false;

  // Match C# tray menu's PresetColors and widths
  static const _colorNames = ['红色', '蓝色', '绿色', '橙色', '紫色', '黑色', '白色'];
  static const _colorValues = [
    0xFFDC1E1E, 0xFF1E78DC, 0xFF1EB43C, 0xFFF0A014,
    0xFFA050DC, 0xFF141414, 0xFFFFFFFF,
  ];
  static const _widthNames = ['极细', '很细', '细', '中', '粗', '很粗', '超粗', '极粗'];

  // Content tab state
  List<_PageInfo> _pages = [];
  List<_PageInfo> _filteredPages = [];
  bool _pagesLoading = false;
  bool _searchLoading = false;
  Timer? _searchDebounce;
  final _searchController = TextEditingController();
  final _thumbnailCache = <int, Uint8List>{};
  final _loadingThumbnails = <int>{};

  // Debug info
  String _dbDebugInfo = '';

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _tabController.addListener(_onTabChanged);
    _bridge = createBridge();
    _bridge.onSettingsChanged(_onSettingsChanged);
    _loadSettings();

    if (Platform.isMacOS) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _resizeToFit());
    }
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchController.dispose();
    _searchDebounce?.cancel();
    _ocrController.dispose();
    _bridge.dispose();
    super.dispose();
  }

  void _onTabChanged() {
    if (_tabController.index == 1 && _pages.isEmpty && !_pagesLoading) {
      _loadPages();
    }
    if (_tabController.index == 1) {
      _dbCheckDebug();
    }
    if (Platform.isMacOS) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _resizeToFit());
    }
  }

  /// Measure the scroll content and tell the host to resize the window to fit.
  void _resizeToFit() {
    final box = _columnKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null) return;
    const width = 600.0;
    const vPadding = 48.0;
    final height = (box.size.height + vPadding).ceilToDouble();
    _channel.invokeMethod('setWindowSize', {
      'width': width,
      'height': height < 540 ? 540 : height,
    });
  }

  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    if (mounted) {
      setState(() {
        _selectedColor = s['color'] ?? _selectedColor;
        _selectedWidth = s['width'] ?? _selectedWidth;
        _smooth = s['smooth'] ?? _smooth;
        _invert = s['invert'] ?? _invert;
        _pressureMonitor = s['pressureMonitor'] ?? _pressureMonitor;
        _showGrid = s['grid'] ?? _showGrid;
      });
    }
  }

  Future<void> _loadSettings() async {
    try {
      final settings = await _bridge.getSettings();
      if (mounted && settings.isNotEmpty) {
        setState(() {
          _selectedColor = settings['color'] ?? 0;
          _selectedWidth = settings['width'] ?? 2;
          _smooth = settings['smooth'] ?? true;
          _invert = settings['invert'] ?? false;
          _pressureMonitor = settings['pressureMonitor'] ?? false;
          _showGrid = settings['grid'] ?? false;
          _connected = true;
        });
        if (Platform.isMacOS) {
          WidgetsBinding.instance.addPostFrameCallback((_) => _resizeToFit());
        }
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
      final json = Platform.isWindows
          ? (await _bridge.invokeMethod('listPages', {}) ?? '[]')
          : (await _channel.invokeMethod<String>('listPages') ?? '[]');
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

  Future<void> _dbCheckDebug() async {
    try {
      final info = StringBuffer();
      info.writeln('--- DB Debug ---');
      // Direct invokeMethod returning raw JSON
      final raw = Platform.isWindows
          ? (await _bridge.invokeMethod('listPages', {}) ?? 'null')
          : (await _channel.invokeMethod<String>('listPages') ?? 'null');
      info.writeln('listPages raw: $raw');
      // Try to parse
      try {
        final list = jsonDecode(raw) as List;
        info.writeln('parsed count: ${list.length}');
        if (list.isNotEmpty) {
          info.writeln('first item: ${list[0]}');
        }
      } catch (e) {
        info.writeln('parse error: $e');
      }
      // Check which pipe server responds
      if (Platform.isWindows) {
        final test = await _bridge.invokeMethod('ping', {});
        info.writeln('ping: $test');
      }
      info.writeln('platform: ${Platform.operatingSystem}');
      setState(() => _dbDebugInfo = info.toString());
    } catch (e) {
      setState(() => _dbDebugInfo = 'Debug error: $e');
    }
  }

  Future<void> _loadThumbnail(_PageInfo page) async {
    if (_thumbnailCache.containsKey(page.id)) {
      page.thumbnail = _thumbnailCache[page.id];
      return;
    }
    try {
      if (Platform.isWindows) {
        final json = await _bridge.invokeMethod('getPageThumbnail', {
          'screenId': page.id,
          'w': page.w,
          'h': page.h,
          'maxSize': 280,
        });
        if (json != null && json.isNotEmpty && mounted) {
          final bytes = base64Decode(json);
          _thumbnailCache[page.id] = bytes;
          page.thumbnail = bytes;
          setState(() {});
        }
      } else {
        final bytes = await _channel.invokeMethod<Uint8List>('getPageThumbnail', {
          'screenId': page.id,
          'w': page.w,
          'h': page.h,
          'maxSize': 280,
        });
        if (bytes != null && bytes.isNotEmpty && mounted) {
          _thumbnailCache[page.id] = bytes;
          page.thumbnail = bytes;
          setState(() {});
        }
      }
    } catch (e) {
      debugPrint('[Content] thumbnail error for page ${page.id}: $e');
    } finally {
      _loadingThumbnails.remove(page.id);
    }
  }

  void _onSearchChanged(String query) {
    _searchDebounce?.cancel();
    _searchDebounce = Timer(const Duration(milliseconds: 300), () {
      _performSearch(query);
    });
  }

  Future<void> _performSearch(String query) async {
    if (query.trim().isEmpty) {
      setState(() => _filteredPages = List.from(_pages));
      return;
    }
    setState(() => _searchLoading = true);
    try {
      final json = Platform.isWindows
          ? (await _bridge.invokeMethod('searchText', {'query': query.trim()}) ?? '[]')
          : (await _channel.invokeMethod<String>('searchText', {'query': query.trim()}) ?? '[]');
      final list = jsonDecode(json) as List<dynamic>;
      if (mounted) {
        setState(() {
          _filteredPages = list.map((e) => _PageInfo.fromJson(e as Map<String, dynamic>)).toList();
          _searchLoading = false;
        });
      }
    } catch (e) {
      debugPrint('[Content] search error: $e');
      if (mounted) setState(() => _searchLoading = false);
    }
  }

  String _ocrPreview(String? text, {int maxLen = 80}) {
    if (text == null || text.isEmpty) return '(无识别文本)';
    final oneLine = text.replaceAll(RegExp(r'\s+'), ' ');
    if (oneLine.length <= maxLen) return oneLine;
    return '${oneLine.substring(0, maxLen)}…';
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
          tabs: const [
            Tab(text: '设置'),
            Tab(text: '内容'),
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
                  _buildSection('Color', _buildColorGrid()),
                  const SizedBox(height: 16),
                  _buildSection('Width', _buildWidthRow()),
                  const SizedBox(height: 16),
                  _buildSection('Actions', _buildActionButtons()),
                  const SizedBox(height: 16),
                  _buildSection('Options', _buildToggles()),
                  const SizedBox(height: 16),
                  _buildSection('Export', _buildExportButtons()),
                  const SizedBox(height: 16),
                  if (Platform.isMacOS) _buildSection('OCR 文字识别', _buildOcrRow()),
                  if (!Platform.isMacOS) _buildSection('OCR 文字识别', _buildOcrRowWindows()),
                ],
              ),
            ),
            // ── Content tab ──
            _buildContentTab(),
          ],
        ),
      );
  }

  Widget _buildContentTab() {
    return Column(
      children: [
        // Search bar
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
          child: TextField(
            controller: _searchController,
            onChanged: _onSearchChanged,
            decoration: InputDecoration(
              hintText: '搜索文本…',
              prefixIcon: const Icon(Icons.search, size: 20),
              suffixIcon: _searchLoading
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: Padding(
                        padding: EdgeInsets.all(14),
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                    )
                  : (_searchController.text.isNotEmpty
                      ? IconButton(
                          icon: const Icon(Icons.clear, size: 18),
                          onPressed: () {
                            _searchController.clear();
                            _performSearch('');
                          },
                        )
                      : null),
              border: const OutlineInputBorder(),
              contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              isDense: true,
            ),
            style: const TextStyle(fontSize: 14),
          ),
        ),
        const SizedBox(height: 8),
        // Debug info
        if (_dbDebugInfo.isNotEmpty)
          Container(
            width: double.infinity,
            color: Colors.yellow.shade50,
            padding: const EdgeInsets.all(8),
            child: Text(_dbDebugInfo, style: const TextStyle(fontSize: 10, fontFamily: 'monospace')),
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
          if (Platform.isWindows) {
            _bridge.invokeMethod('navigateToPage', {'screenId': page.id});
          } else {
            _channel.invokeMethod('navigateToPage', {'screenId': page.id});
          }
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
                        const SizedBox(height: 2),
                        Text(_ocrPreview(page.ocr, maxLen: 40),
                            style: TextStyle(fontSize: 11, color: Colors.grey.shade600),
                            maxLines: 2, overflow: TextOverflow.ellipsis),
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
      final ok = Platform.isWindows
          ? (await _bridge.invokeMethod('deletePage', {'screenId': page.id}) ?? '0') == '1'
          : await _channel.invokeMethod<int>('deletePage', {'screenId': page.id}) == 1;
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
        ElevatedButton.icon(
          onPressed: () {
            _setSetting('export_pdf', true);
          },
          icon: const Icon(Icons.picture_as_pdf, size: 16),
          label: const Text('导出 PDF', style: TextStyle(fontSize: 15)),
          style: ElevatedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
      ],
    );
  }

  Widget _buildToggles() {
    return Column(
      children: [
        SwitchListTile(
          title: const Text('笔迹美化 (去抖)', style: TextStyle(fontSize: 15)),
          value: _smooth,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _smooth = v);
            _setSetting('smooth', v);
          },
        ),
        SwitchListTile(
          title: const Text('坐标翻转 (180°)', style: TextStyle(fontSize: 15)),
          value: _invert,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _invert = v);
            _setSetting('invert', v);
          },
        ),
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
      ],
    );
  }

  Widget _buildExportButtons() {
    // Animated GIF is macOS-only (uses ObjC). PDF works on all platforms.
    final children = <Widget>[];
    if (Platform.isMacOS) {
      children.add(const Text(
        '将当前笔迹按笔顺生成为动画 GIF，自动复制到剪贴板并保存到桌面。',
        style: TextStyle(fontSize: 13, color: Colors.grey),
      ));
      children.add(const SizedBox(height: 8));
      children.add(FilledButton.icon(
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
      ));
      children.add(const SizedBox(height: 8));
    }
    children.add(FilledButton.icon(
      icon: _pdfExporting
          ? const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
            )
          : const Icon(Icons.picture_as_pdf, size: 18),
      label: Text(_pdfExporting ? '导出中…' : '导出全部页面为 PDF'),
      onPressed: _pdfExporting ? null : _exportPdf,
    ));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }

  Widget _buildOcrRowWindows() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        OutlinedButton.icon(
          onPressed: _ocrBackfilling ? null : _ocrBackfill,
          icon: _ocrBackfilling
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.storage, size: 16),
          label: Text(_ocrBackfilling ? '补全中…' : '补全所有页面 OCR'),
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
      ],
    );
  }

  Widget _buildOcrRow() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: double.infinity,
          child: TextField(
            controller: _ocrController,
            readOnly: true,
            maxLines: 3,
            minLines: 1,
            decoration: const InputDecoration(
              hintText: '识别结果将显示在这里',
              border: OutlineInputBorder(),
              contentPadding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            style: const TextStyle(fontSize: 15),
          ),
        ),
        const SizedBox(height: 8),
        ElevatedButton.icon(
          onPressed: _ocrLoading ? null : _recognizeText,
          icon: _ocrLoading
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.text_snippet, size: 16),
          label: Text(_ocrLoading ? '识别中…' : '识别笔迹'),
          style: ElevatedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
        const SizedBox(height: 8),
        OutlinedButton.icon(
          onPressed: _ocrBackfilling ? null : _ocrBackfill,
          icon: _ocrBackfilling
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.storage, size: 16),
          label: Text(_ocrBackfilling ? '补全中…' : '补全所有页面 OCR'),
          style: OutlinedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
      ],
    );
  }

  bool _ocrLoading = false;
  bool _ocrBackfilling = false;
  final _ocrController = TextEditingController();

  Future<void> _recognizeText() async {
    setState(() => _ocrLoading = true);
    try {
      final text = Platform.isWindows
          ? (await _bridge.invokeMethod('recognizeText', {}) ?? '')
          : (await _channel.invokeMethod<String>('recognizeText') ?? '');
      if (mounted) {
        _ocrController.text = text;
        if (text.isEmpty) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('未识别到文字'), duration: Duration(seconds: 2)),
          );
        }
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('识别失败: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _ocrLoading = false);
    }
  }

  Future<void> _ocrBackfill() async {
    setState(() => _ocrBackfilling = true);
    try {
      _setSetting('ocr_backfill', true);
      await Future.delayed(const Duration(seconds: 3));
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('OCR 补全完成'), duration: Duration(seconds: 2)),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('补全失败: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _ocrBackfilling = false);
    }
  }

  bool _gifExporting = false;

  Future<void> _exportAnimatedGif() async {
    setState(() => _gifExporting = true);
    try {
      final ok = Platform.isWindows
          ? (await _bridge.invokeMethod('exportAnimatedGif', {}) == 'true')
          : (await _channel.invokeMethod<bool>('exportAnimatedGif') == true);
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
      _setSetting('export_pdf', true);
      // Allow a moment for Rust to export, then give feedback
      await Future.delayed(const Duration(milliseconds: 500));
      if (mounted) {
        setState(() => _pdfExporting = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: const Text('PDF 已保存到桌面'),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() => _pdfExporting = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('PDF 导出失败: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _pdfExporting = false);
    }
  }
}
