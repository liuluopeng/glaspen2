import 'dart:async';
import 'dart:convert';
import 'dart:io';

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

/// Windows: uses Named Pipe for IPC with the main overlay process.
class _NamedPipeBridge extends _SettingsBridge {
  RandomAccessFile? _pipe;
  StreamSubscription<List<int>>? _sub;
  final _buffer = <int>[];
  void Function(Map<dynamic, dynamic>)? _onChanged;
  bool _connected = false;
  Timer? _reconnectTimer;

  _NamedPipeBridge() {
    _connect();
  }

  void _connect() {
    try {
      final file = File(_pipeName);
      // Open for read+write
      _pipe = file.openSync(mode: FileMode.write);
      _connected = true;
      debugPrint('[Settings] Connected to pipe $_pipeName');

      // Listen for push notifications from the host
      // Use a separate read stream
      _startListening();
    } catch (e) {
      debugPrint('[Settings] Pipe connect failed: $e — retrying in 2s');
      _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
    }
  }

  void _startListening() {
    // Named pipes on Windows: we need to read asynchronously
    // Open a separate handle for reading
    try {
      final readFile = File(_pipeName).openSync(mode: FileMode.read);
      readFile.listen(
        (data) {
          _buffer.addAll(data);
          _processBuffer();
        },
        onDone: () {
          debugPrint('[Settings] Pipe read closed');
          _connected = false;
          _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
        },
        onError: (e) {
          debugPrint('[Settings] Pipe read error: $e');
          _connected = false;
        },
      );
    } catch (e) {
      debugPrint('[Settings] Pipe read open failed: $e');
    }
  }

  void _processBuffer() {
    // Messages are newline-delimited JSON
    while (true) {
      final newlineIdx = _buffer.indexOf(10); // \n
      if (newlineIdx < 0) break;
      final line = utf8.decode(_buffer.sublist(0, newlineIdx));
      _buffer.removeRange(0, newlineIdx + 1);
      try {
        final msg = jsonDecode(line) as Map<String, dynamic>;
        if (msg['type'] == 'onSettingsChanged' && _onChanged != null) {
          _onChanged!(msg['data'] as Map<dynamic, dynamic>);
        }
      } catch (e) {
        debugPrint('[Settings] Parse error: $e');
      }
    }
  }

  Future<Map<String, dynamic>> _sendRequest(Map<String, dynamic> req) async {
    if (!_connected || _pipe == null) {
      throw Exception('Not connected');
    }
    final data = utf8.encode(jsonEncode(req) + '\n');
    _pipe!.writeFromSync(data);
    // For request/response, we'd need a bidirectional pipe.
    // For simplicity, use synchronous approach with getSettings via shared memory/file.
    // Actually, named pipes support bidirectional communication.
    // But RandomAccessFile doesn't support async reads well on Windows.
    // Let's use a simpler approach: write request, read response.
    throw UnimplementedError('Use getSettings/setSetting directly');
  }

  @override
  Future<Map<dynamic, dynamic>> getSettings() async {
    if (!_connected || _pipe == null) {
      return {};
    }
    try {
      final req = jsonEncode({'type': 'getSettings'}) + '\n';
      _pipe!.writeFromSync(utf8.encode(req));
      // Read response (blocking)
      // This is tricky with RandomAccessFile...
      // For now, use a workaround: read until newline
      final responseBytes = <int>[];
      while (true) {
        final byte = _pipe!.readByteSync();
        if (byte < 0 || byte == 10) break;
        responseBytes.add(byte);
      }
      final response = jsonDecode(utf8.decode(responseBytes));
      if (response is Map && response['type'] == 'getSettings_response') {
        return response['data'] as Map<dynamic, dynamic>;
      }
      return {};
    } catch (e) {
      debugPrint('[Settings] getSettings error: $e');
      return {};
    }
  }

  @override
  Future<void> setSetting(String key, dynamic value) async {
    if (!_connected || _pipe == null) return;
    try {
      final req = jsonEncode({'type': 'setSetting', 'key': key, 'value': value}) + '\n';
      _pipe!.writeFromSync(utf8.encode(req));
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
    _sub?.cancel();
    _pipe?.close();
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
  bool _outline = false;
  bool _inverse = false;
  bool _rainbow = false;
  bool _launchAtLogin = false;
  bool _frostedGlass = false;
  bool _connected = false;

  static const _colorNames = [
    'Red', 'Orange', 'Yellow', 'Green', 'Cyan',
    'Blue', 'Purple', 'Pink', 'White', 'Black',
  ];
  static const _colorValues = [
    0xFFFF0000, 0xFFFF8C00, 0xFFFFD700, 0xFF00AA00, 0xFF00CCCC,
    0xFF0000FF, 0xFF8B00FF, 0xFFFF69B4, 0xFFFFFFFF, 0xFF000000,
  ];
  static const _widthNames = ['Fine', 'Thin', 'Medium', 'Thick', 'Bold'];

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
        _outline = s['outline'] ?? _outline;
        _inverse = s['inverse'] ?? _inverse;
        _rainbow = s['rainbow'] ?? _rainbow;
        _launchAtLogin = s['launchAtLogin'] ?? _launchAtLogin;
        _frostedGlass = s['frostedGlass'] ?? _frostedGlass;
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
          _outline = settings['outline'] ?? false;
          _inverse = settings['inverse'] ?? false;
          _rainbow = settings['rainbow'] ?? false;
          _launchAtLogin = settings['launchAtLogin'] ?? false;
          _frostedGlass = settings['frostedGlass'] ?? false;
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
      children: List.generate(10, (i) {
        final selected = i == _selectedColor;
        final isWhite = i == 8;
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
      children: List.generate(5, (i) {
        final selected = i == _selectedWidth;
        return SizedBox(
          width: 60,
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

  Widget _buildToggles() {
    return Column(
      children: [
        SwitchListTile(
          title: const Text('Outline', style: TextStyle(fontSize: 13)),
          value: _outline,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _outline = v);
            _setSetting('outline', v);
          },
        ),
        SwitchListTile(
          title: const Text('Inverse Color', style: TextStyle(fontSize: 13)),
          value: _inverse,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _inverse = v);
            _setSetting('inverse', v);
          },
        ),
        SwitchListTile(
          title: const Text('Rainbow', style: TextStyle(fontSize: 13)),
          value: _rainbow,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _rainbow = v);
            _setSetting('rainbow', v);
          },
        ),
        SwitchListTile(
          title: const Text('Launch at Login', style: TextStyle(fontSize: 13)),
          value: _launchAtLogin,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _launchAtLogin = v);
            _setSetting('launchAtLogin', v);
          },
        ),
        SwitchListTile(
          title: const Text('Frosted Glass', style: TextStyle(fontSize: 13)),
          value: _frostedGlass,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _frostedGlass = v);
            _setSetting('frostedGlass', v);
          },
        ),
      ],
    );
  }
}
