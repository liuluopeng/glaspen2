import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

// GetLastError from kernel32
final _getLastError = DynamicLibrary.open('kernel32.dll')
    .lookupFunction<Uint32 Function(), int Function()>('GetLastError');

void main() => runApp(const GlaspenSettingsApp());

// ── Platform-specific communication ──

const _channel = MethodChannel('com.glaspen/settings');
const _pipeName = r'\\.\pipe\glaspen2_settings';

/// Abstract interface for settings communication.
abstract class _SettingsBridge {
  Future<Map<dynamic, dynamic>> getSettings();
  Future<void> setSetting(String key, dynamic value);
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

class _SettingsPageState extends State<SettingsPage> {
  late _SettingsBridge _bridge;
  int _selectedColor = 0;
  int _selectedWidth = 2;
  bool _smooth = true;
  bool _invert = false;
  bool _connected = false;

  // Match C# tray menu's PresetColors and widths
  static const _colorNames = ['红色', '蓝色', '绿色', '橙色', '紫色', '黑色', '白色'];
  static const _colorValues = [
    0xFFDC1E1E, 0xFF1E78DC, 0xFF1EB43C, 0xFFF0A014,
    0xFFA050DC, 0xFF141414, 0xFFFFFFFF,
  ];
  static const _widthNames = ['极细', '很细', '细', '中', '粗', '很粗', '超粗', '极粗'];

  @override
  void initState() {
    super.initState();
    _bridge = createBridge();
    _bridge.onSettingsChanged(_onSettingsChanged);
    _loadSettings();
  }

  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    if (mounted) {
      setState(() {
        _selectedColor = s['color'] ?? _selectedColor;
        _selectedWidth = s['width'] ?? _selectedWidth;
        _smooth = s['smooth'] ?? _smooth;
        _invert = s['invert'] ?? _invert;
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
          _connected = true;
        });
      }
    } catch (_) {
      // Fallback: use defaults if bridge not available
    }
  }

  void _setSetting(String key, dynamic value) {
    _bridge.setSetting(key, value);
  }

  @override
  void dispose() {
    _bridge.dispose();
    super.dispose();
  }

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
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _buildSection('Color', _buildColorGrid()),
            const SizedBox(height: 16),
            _buildSection('Width', _buildWidthRow()),
            const SizedBox(height: 16),
            _buildSection('Actions', _buildActionButtons()),
            const SizedBox(height: 16),
            _buildSection('Options', _buildToggles()),
          ],
        ),
      ),
    );
  }

  Widget _buildSection(String title, Widget child) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title,
            style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13)),
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
                fontSize: 11,
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
                style: const TextStyle(fontSize: 11)),
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
          label: const Text('撤销上一笔', style: TextStyle(fontSize: 13)),
          style: ElevatedButton.styleFrom(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          ),
        ),
        ElevatedButton.icon(
          onPressed: () {
            _setSetting('export_animated_gif', true);
          },
          icon: const Icon(Icons.gif, size: 16),
          label: const Text('导出动画 GIF', style: TextStyle(fontSize: 13)),
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
          title: const Text('笔迹美化 (去抖)', style: TextStyle(fontSize: 13)),
          value: _smooth,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _smooth = v);
            _setSetting('smooth', v);
          },
        ),
        SwitchListTile(
          title: const Text('坐标翻转 (180°)', style: TextStyle(fontSize: 13)),
          value: _invert,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _invert = v);
            _setSetting('invert', v);
          },
        ),
      ],
    );
  }
}
