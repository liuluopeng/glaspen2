import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

void main() => runApp(const GlaspenSettingsApp());

// Method channel to communicate with Rust/ObjC host
const _channel = MethodChannel('com.glaspen/settings');

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
  int _selectedColor = 0;
  int _selectedWidth = 2;
  bool _outline = false;
  bool _inverse = false;
  bool _rainbow = false;
  bool _launchAtLogin = false;
  bool _frostedGlass = false;

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
    _loadSettings();
    // Listen for settings changes from ObjC host
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'onSettingsChanged') {
        final Map<dynamic, dynamic> s = call.arguments;
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
    });
  }

  Future<void> _loadSettings() async {
    try {
      final Map<dynamic, dynamic> settings =
          await _channel.invokeMethod('getSettings');
      if (mounted) {
        setState(() {
          _selectedColor = settings['color'] ?? 0;
          _selectedWidth = settings['width'] ?? 2;
          _outline = settings['outline'] ?? false;
          _inverse = settings['inverse'] ?? false;
          _rainbow = settings['rainbow'] ?? false;
          _launchAtLogin = settings['launchAtLogin'] ?? false;
          _frostedGlass = settings['frostedGlass'] ?? false;
        });
      }
    } catch (_) {
      // Fallback: use defaults if channel not available
    }
  }

  void _setSetting(String key, dynamic value) {
    _channel.invokeMethod('setSetting', {'key': key, 'value': value});
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
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
            const SizedBox(height: 16),
            _buildSection('Opacity', _buildOpacity()),
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

  Widget _buildOpacity() {
    return Row(
      children: [
        const Text('50%', style: TextStyle(fontSize: 13)),
        const SizedBox(width: 12),
        OutlinedButton(
          onPressed: () => _setSetting('opacity', 0.5),
          style: OutlinedButton.styleFrom(
            minimumSize: const Size(58, 30),
            padding: EdgeInsets.zero,
            shape:
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(4)),
          ),
          child: const Text('50%', style: TextStyle(fontSize: 11)),
        ),
      ],
    );
  }
}
