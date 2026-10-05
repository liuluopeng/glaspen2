import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart'
    show PointerScrollEvent, PointerSignalEvent;
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart' as frb;

import 'src/rust/api.dart' as rust;
import 'src/rust/frb_generated.dart';

part 'main.bridge.dart';
part 'main.models.dart';

void main() {
  // FRB 需要绑定初始化(消息端口/isolate)后才能解析 Rust 符号
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const GlaspenSettingsApp());
}

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

/// Abstract interface for settings communication.
///
/// macOS 用 flutter_rust_bridge 直接调同进程内的 Rust(`_FrbBridge`),
/// Windows 的面板是独立进程,只能走命名管道(`_NamedPipeBridge`)。
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
  late SettingsBridge _bridge;
  late TabController _tabController;
  int _selectedColor = 0;
  int _selectedWidth = 2;
  bool _pressureMonitor = false;
  bool _showGrid = false;
  bool _gridFollowStrokes = false;
  bool _glassFollowStrokes = true;
  bool _softShadow = false;
  bool _invertInk = false;
  int _invertFps = 30;
  bool _outlineEnabled = false;
  bool _infiniteCanvas = false;
  int _gridSizeValue = 40; // 网格大小(逻辑 px)
  int _gridDividerValue = 0; // 网格分栏:0=无 1=左右两栏 2=上下两栏 3=九宫格
  bool _minimapEnabled = false;
  // 缩略图按需加载:只加载可视区域的缩略图,滚动时按需补充
  final ScrollController _gridScroll = ScrollController();
  Timer? _gridScrollDebounce;
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
  // 手写消息集成总开关(默认关;关 = 热键直通、登录/共享界面收起)
  bool _chatIntegration = false;
  // 「自由涂鸦」tab 显隐(默认关:最小面板只有 设置+活页本)
  bool _showFreeCanvas = false;
  // 共享画布上行开关(活页本 tab;仅集成开启时可见/生效)
  bool _shareCanvas = false;

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
  static const _colorNames = ['红色', '橙色', '黄色', '绿色', '青色', '蓝色', '紫色', '粉色', '白色', '黑色'];
  static const _colorValues = [
    0xFFFF0038, 0xFFFF4D00, 0xFFFFCC00, 0xFF00E676, 0xFF00A3FF,
    0xFF0096FF, 0xFFAA00FF, 0xFFFF0080, 0xFFFFFFFF, 0xFF000000,
  ];
  static const _widthNames = ['极细', '很细', '细', '中', '粗', '很粗', '超粗', '极粗'];

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
  /// OCR 搜索(本子页视图内): 激活后网格 = 全库匹配页(跨组)
  bool _searchMode = false;
  bool _searchFieldVisible = false;
  String _searchText = '';
  int _flipEffect = 0; // 翻页动效: 0=macOS 系统 1=时光隧道
  bool _pagesLoading = false;
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
    _bridge = createBridge();
    _bridge.onSettingsChanged(_onSettingsChanged);
    // Windows 管道连接成功后重新拉取设置(macOS 通道立即可用,不影响)
    _bridge.onConnected = () => _loadSettings();
    _loadSettings();
    // 「关于」区显示当前版本;取不到只显示 —,不影响面板其它功能
    unawaited(_loadAppVersion());
  }

  @override
  void dispose() {
    _gridScrollDebounce?.cancel();
    _gridScroll.removeListener(_onGridScroll);
    _gridScroll.dispose();
    _tabController.dispose();
    _reloadTimer?.cancel();
    _updSub?.cancel(); // 还在下载时销毁面板 = 取消下载
    _chatApiBase.dispose();
    _chatUser.dispose();
    _chatPassword.dispose();
    _bridge.dispose();
    super.dispose();
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
    final idx = _tabController.index;
    // 切 tab 同时切换画布模式:活页本=翻页模式,自由涂鸦=无限画布。
    // 本地状态必须立即更新:否则再点回原 tab 时,Flutter 以为模式没变而不发消息。
    if (idx == 1 && _infiniteCanvas) {
      setState(() => _infiniteCanvas = false);
      _setSetting('infiniteCanvas', false);
    } else if (idx == 2 && _showFreeCanvas && !_infiniteCanvas) {
      setState(() => _infiniteCanvas = true);
      _setSetting('infiniteCanvas', true);
    }
    if (idx == 1 && _pages.isEmpty && !_pagesLoading) {
      _loadPages();
    }
    if (idx == 2 && _showFreeCanvas) {
      _loadCanvasOverview();
    }
  }

  /// tab 数量跟随「自由涂鸦」开关(开 = 3 个;默认关 = 2 个:设置+活页本)。
  /// 长度变化必须重建 controller,否则 TabBar/TabBarView 断言崩溃。
  void _syncTabCount() {
    final want = _showFreeCanvas ? 3 : 2;
    if (_tabController.length == want) return;
    final old = _tabController;
    _tabController = TabController(length: want, vsync: this);
    _tabController.addListener(() {
      if (!mounted) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _updateVisibleRange();
      });
    });
    _tabController.addListener(_onTabChanged);
    old.dispose();
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
        _glassFollowStrokes = s['glassFollowStrokes'] as bool? ?? _glassFollowStrokes;
        _softShadow = _b(s['softShadow']);
        _invertInk = _b(s['invertInk']);
        _invertFps = (s['invertFps'] as num?)?.toInt() ?? _invertFps;
        _outlineEnabled = _b(s['outline']);
        _infiniteCanvas = _b(s['infiniteCanvas']);
        _minimapEnabled = _b(s['minimap']);
        _gridSizeValue = (s['gridSize'] as num?)?.toInt() ?? _gridSizeValue;
        _gridDividerValue = (s['gridDivider'] as num?)?.toInt() ?? _gridDividerValue;
        _gifFps = (s['gifFps'] as num?)?.toInt() ?? _gifFps;
        _gifResolution = (s['gifResolution'] as num?)?.toDouble() ?? _gifResolution;
        _gifSpeed = (s['gifSpeed'] as num?)?.toDouble() ?? _gifSpeed;
        _gifEndMode = (s['gifEndMode'] as num?)?.toInt() ?? _gifEndMode;
        _flipEffect = (s['flipEffect'] as num?)?.toInt() ?? _flipEffect;
        _showFreeCanvas = _b(s['showFreeCanvas']);
        _shareCanvas = _b(s['shareCanvas']);
        _syncTabCount();
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
          _glassFollowStrokes =
              settings['glassFollowStrokes'] as bool? ?? _glassFollowStrokes;
          _softShadow = _b(settings['softShadow']);
          _invertInk = _b(settings['invertInk']);
          _invertFps = (settings['invertFps'] as num?)?.toInt() ?? _invertFps;
          _infiniteCanvas = _b(settings['infiniteCanvas']);
          _minimapEnabled = _b(settings['minimap']);
          _gridSizeValue = (settings['gridSize'] as num?)?.toInt() ?? _gridSizeValue;
          _gridDividerValue = (settings['gridDivider'] as num?)?.toInt() ?? _gridDividerValue;
          _gifFps = (settings['gifFps'] as num?)?.toInt() ?? 15;
          _gifResolution = (settings['gifResolution'] as num?)?.toDouble() ?? 0.5;
          _gifSpeed = (settings['gifSpeed'] as num?)?.toDouble() ?? 2.0;
          _gifEndMode = (settings['gifEndMode'] as num?)?.toInt() ?? 1;
          _flipEffect = (settings['flipEffect'] as num?)?.toInt() ?? _flipEffect;
          _chatApiBase.text = (settings['chatApiBase'] as String?) ?? '';
          _chatUser.text = (settings['chatUser'] as String?) ?? '';
          _chatHasPassword = _b(settings['chatHasPassword']);
          _chatIntegration = _b(settings['chatIntegration']);
          _showFreeCanvas = _b(settings['showFreeCanvas']);
          _shareCanvas = _b(settings['shareCanvas']);
          _syncTabCount();
          _connected = true;
        });
        // 设置就绪后顺手预取活页本首屏缩略图(一次批量往返),
        // 等用户切到活页本时直接就有图,连加载态都不用出现。
        if (_pages.isEmpty && !_pagesLoading) unawaited(_loadPages());
      } else if (mounted) {
        // Windows 管道未就绪时 getSettings 返回空:稍后重试
        _reloadTimer = Timer(const Duration(seconds: 2), _loadSettings);
      }
    } catch (e) {
      // 桥不可用时退回默认值;FRB 初始化失败也会走这里,打日志便于定位
      // (Rust 符号没导出、库没链接等都会在首次调用时暴露出来)
      debugPrint('[Settings] load failed: $e');
    }
  }

  void _setSetting(String key, dynamic value) {
    // 这是"发出去就不管"的写入:失败只记日志,不该因为桥异常打断 UI
    _bridge.setSetting(key, value).catchError((Object e) {
      debugPrint('[Settings] set "$key" failed: $e');
    });
  }

  // ── 涂鸦身份(手写消息登录 axum)──

  /// 保存三个字段;密码为空 = 保持已存的(面板不回显密码)。
  Future<void> _saveChatAuth() async {
    await _bridge.setSetting('chatApiBase', _chatApiBase.text.trim());
    await _bridge.setSetting('chatUser', _chatUser.text.trim());
    if (_chatPassword.text.isNotEmpty) {
      await _bridge.setSetting('chatPassword', _chatPassword.text);
    }
  }

  /// 「测试登录」:先保存,再强制登录一次,回显结果。
  Future<void> _testChatLogin() async {
    setState(() => _chatLoginBusy = true);
    try {
      await _saveChatAuth();
      final err = await _bridge.testChatLogin();
      if (!mounted) return;
      setState(() {
        _chatHasPassword = _chatHasPassword || _chatPassword.text.isNotEmpty;
        _chatLoginMsg =
            err.isEmpty ? '登录成功,手写消息将携带身份' : '登录失败:$err';
      });
    } catch (e) {
      if (mounted) setState(() => _chatLoginMsg = '登录失败:$e');
    } finally {
      if (mounted) setState(() => _chatLoginBusy = false);
    }
  }

  Widget _buildChatAuthSection() {
    const faint12 = TextStyle(fontSize: 12, color: _inkFaint);
    // 总开关:关 = 只剩开关本身;⌘⌃2/⌘⌃3 直通,不显登录与共享。
    final master = _tile(SwitchListTile(
      title: const Text('启用手写消息与共享',
          style: TextStyle(fontSize: 14)),
      subtitle: const Text('关闭时不占用 ⌘⌃2/⌘⌃3,涂鸦功能不受影响',
          style: TextStyle(fontSize: 12)),
      value: _chatIntegration,
      dense: true,
      contentPadding: EdgeInsets.zero,
      onChanged: (v) {
        setState(() => _chatIntegration = v);
        _setSetting('chatIntegration', v);
      },
    ));
    if (!_chatIntegration) {
      return Column(crossAxisAlignment: CrossAxisAlignment.start,
          children: [master]);
    }

    const faint = TextStyle(fontSize: 13, color: _inkFaint);
    InputDecoration deco(String label, String hint) => InputDecoration(
          labelText: label,
          hintText: hint,
          isDense: true,
          border: const OutlineInputBorder(),
        );
    final Widget status;
    if (_chatLoginBusy) {
      status = const Row(children: [
        SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2)),
        SizedBox(width: 8),
        Text('正在登录…', style: faint),
      ]);
    } else if (_chatLoginMsg.isNotEmpty) {
      final ok = _chatLoginMsg.startsWith('登录成功');
      status = Text(_chatLoginMsg,
          style: TextStyle(
              fontSize: 12.5, color: ok ? const Color(0xFF00B16E) : _penRed));
    } else if (_chatHasPassword) {
      status = const Text('已配置账号(⌘⌃2/⌘⌃3 手写消息将携带身份)',
          style: faint);
    } else {
      status = const Text('未配置:手写消息功能仍可用,只是发送时不带身份',
          style: faint);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        master,
        const SizedBox(height: 4),
        TextField(
          controller: _chatApiBase,
          decoration: deco('服务地址', '例如 https://192.168.31.58:23001'),
          style: const TextStyle(fontSize: 13),
          autocorrect: false,
          keyboardType: TextInputType.url,
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _chatUser,
          decoration: deco('用户名', ''),
          style: const TextStyle(fontSize: 13),
          autocorrect: false,
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _chatPassword,
          obscureText: true,
          decoration: deco('密码', _chatHasPassword ? '已保存(留空保持不变)' : ''),
          style: const TextStyle(fontSize: 13),
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 10),
        Row(children: [
          OutlinedButton(
            onPressed: _chatLoginBusy ? null : _testChatLogin,
            child: const Text('保存并测试登录', style: TextStyle(fontSize: 13)),
          ),
          const SizedBox(width: 12),
          Expanded(child: status),
        ]),
        const SizedBox(height: 6),
        const Text(
          '用于 ⌘⌃2 手写草稿 / ⌘⌃3 直发的作者归属;涂鸦本身不依赖登录。',
          style: faint12,
        ),
      ],
    );
  }

  // ── Content tab ──
  Future<void> _loadPages() async {
    if (_pagesLoading) return;
    setState(() => _pagesLoading = true);
    try {
      final pages = await _bridge.listPages();

      // 首屏缩略图先取回来再发布列表:活页本第一帧就带图,不会先闪一圈占位。
      // 失败或超时只是退回占位,列表照常显示。
      if (pages.isNotEmpty) {
        final ids = pages.take(_thumbInitialCount).map((p) => p.id).toList();
        try {
          final thumbs = await _bridge
              .getPageThumbnails(ids, _thumbMaxSize)
              .timeout(const Duration(seconds: 5), onTimeout: () => const {});
          _thumbnailCache.addAll(thumbs);
          await _precacheThumbnails(thumbs.values);
        } catch (e) {
          debugPrint('[Content] initial thumbnails error: $e');
        }
      }

      if (mounted) {
        setState(() {
          _pages = pages;
          _applyPageList();
          _pagesLoading = false;
          for (final p in _pages) {
            p.thumbnail = _thumbnailCache[p.id];
          }
        });
        // 布局完成后补齐首屏之外的可见页(滚动时同样走批量)
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) _updateVisibleRange();
        });
      }
    } catch (e) {
      debugPrint('[Content] listPages error: $e');
      if (mounted) setState(() => _pagesLoading = false);
    }
  }

  /// 把 PNG 解码进 ImageCache:随后的 setState 首帧就能画出来,
  /// 否则 Image.memory 的异步解码会让占位多显示几帧。
  Future<void> _precacheThumbnails(Iterable<Uint8List> blobs) async {
    for (final bytes in blobs) {
      try {
        await precacheImage(MemoryImage(bytes), context);
      } catch (_) {
        // 单张解码失败不影响其它页
      }
    }
  }

  /// 请求一批缩略图;重复或已在途的页会被跳过,多次触发合并成一次往返。
  void _requestThumbnails(Iterable<int> ids) {
    var added = false;
    for (final id in ids) {
      if (_thumbnailCache.containsKey(id) || !_loadingThumbnails.add(id)) continue;
      _thumbQueue.add(id);
      added = true;
    }
    if (added) unawaited(_flushThumbnailQueue());
  }

  Future<void> _flushThumbnailQueue() async {
    if (_batchRunning || !mounted) return;
    _batchRunning = true;
    try {
      while (_thumbQueue.isNotEmpty && mounted) {
        final ids = _thumbQueue.toList();
        _thumbQueue.clear();
        var got = const <int, Uint8List>{};
        try {
          got = await _bridge
              .getPageThumbnails(ids, _thumbMaxSize)
              .timeout(const Duration(seconds: 5), onTimeout: () => const {});
        } catch (e) {
          debugPrint('[Content] thumbnails error: $e');
        }
        _loadingThumbnails.removeAll(ids);
        if (got.isEmpty) continue;
        _thumbnailCache.addAll(got);
        await _precacheThumbnails(got.values);
        if (!mounted) return;
        // 整批只重建一次,而不是每页一次 setState
        setState(() {
          for (final p in _pages) {
            final bytes = _thumbnailCache[p.id];
            if (bytes != null) p.thumbnail = bytes;
          }
        });
      }
    } finally {
      _batchRunning = false;
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
                  if (_showFreeCanvas)
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
                  const SizedBox(height: 16),
                  _buildSection('涂鸦身份', _buildChatAuthSection()),
                  const SizedBox(height: 16),
                  _buildSection('关于', _buildAboutSection()),
                ],
              ),
            )),
            // ── Content(活页本) tab ──
            _tabBackground('assets/tab_bg_pages.jpg', _buildContentTab()),
            if (_showFreeCanvas)
              _tabBackground('assets/tab_bg_infinite.jpg', _buildCanvasTab()),
          ],
        ),
          ),
        ],
      ),
    );
  }

  /// 活页本 tab 上方是固定设置区(不可滚), 下方页面网格自己滚动;
  /// 把设置区的滚轮事件转发给网格控制器 —— 鼠标悬在设置区也能滚动
  /// 页面列表(悬在网格上则走网格自己的滚动, 不会重复)。
  void _forwardWheelToGrid(PointerSignalEvent event) {
    if (event is! PointerScrollEvent || !_gridScroll.hasClients) return;
    final pos = _gridScroll.position;
    if (pos.maxScrollExtent <= 0) return;
    final target =
        (pos.pixels + event.scrollDelta.dy).clamp(0.0, pos.maxScrollExtent);
    if (target != pos.pixels) _gridScroll.jumpTo(target);
  }

  Widget _buildContentTab() {
    return Column(
      children: [
        // 上方设置区不参与滚动(网格自己滚): 把这里的滚轮事件转发给
        // 网格控制器, 鼠标悬在设置区也能滚动页面列表。
        Listener(
          onPointerSignal: _forwardWheelToGrid,
          child: Column(
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
              if (_chatIntegration)
                Padding(
                  padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
                  child: _buildSection('共享画布', _tile(SwitchListTile(
                    title: const Text('实时发送涂鸦', style: TextStyle(fontSize: 14)),
                    subtitle: const Text('抬笔即推给对方应用当前打开的接收页;关闭即停',
                        style: TextStyle(fontSize: 12)),
                    value: _shareCanvas,
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    onChanged: (v) {
                      setState(() => _shareCanvas = v);
                      _setSetting('shareCanvas', v);
                    },
                  ))),
                ),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
                child: _buildSection('Export', _buildExportButtons()),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
                child: _buildSection('数据备份', _buildBackupButtons()),
              ),
              if (!_pagesLoading && _openNotebook != null && _filteredPages.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: _buildGridToolbar(),
                ),
            ],
          ),
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

  /// 笔记本封面色: 由尺寸 key 决定(确定性"随机") —— 生成一次终生不变,
  /// 任何机器/重装都得到同一本色。平涂色板, GoodNotes 式干净。
  static const _coverPalette = [
    Color(0xFF4E7DC4), // 钴蓝
    Color(0xFFE06A5E), // 珊瑚红
    Color(0xFF4CA88E), // 青玉
    Color(0xFFE0954E), // 杏橙
    Color(0xFF7B68C4), // 长春花
    Color(0xFFC4588A), // 玫红
    Color(0xFF55A8C4), // 青蓝
    Color(0xFF8FAE4E), // 苔绿
  ];
  static Color _coverColor(String key) {
    var hv = 0;
    for (final c in key.codeUnits) {
      hv = (hv * 31 + c) & 0x7FFFFFFF;
    }
    return _coverPalette[hv % _coverPalette.length];
  }

  /// 笔记本网格: GoodNotes 式平涂封面 —— 纯色 + 右侧**松紧带**(黑色竖带,
  /// 本子的签名元素) + 居中白字标题。**卡片比例 = 该本子的真实比例**
  /// (3440×1440 是胖扁的本子, 竖屏分辨率是瘦高的本子)。
  /// 布局: 列数随窗口宽度自适应、卡片铺满不留缝, 同排卡片**底边对齐**
  /// (书架比喻: 胖瘦本子立在架上)。
  Widget _buildNotebookGrid() {
    final notebooks = _notebooks;
    if (notebooks.isEmpty) {
      return const Center(
          child: Text('暂无页面', style: TextStyle(fontSize: 14, color: Colors.grey)));
    }
    const spacing = 10.0, targetW = 300.0, maxCardW = 360.0;
    return LayoutBuilder(builder: (context, box) {
      final availW = box.maxWidth;
      // 列数: 目标宽 ~300, 且不超过本子数(不排空行)
      final cols = ((availW + spacing) / (targetW + spacing))
          .floor()
          .clamp(1, notebooks.length);
      final cardW = ((availW - spacing * (cols - 1)) / cols).clamp(0.0, maxCardW);
      final cards = <Widget>[];
      notebooks.forEach((key, pages) {
        final parts = key.split('x');
        final w = double.parse(parts[0]), h = double.parse(parts[1]);
        final cardH = (cardW * h / w).clamp(110.0, 420.0);
        final cover = _coverColor(key);
        cards.add(SizedBox(
          width: cardW,
          height: cardH,
          child: GestureDetector(
            onTap: () => setState(() {
              _openNotebook = key;
              _multiSelect = false;
              _selectedPageIds.clear();
              _applyPageList();
            }),
            child: Card(
              clipBehavior: Clip.antiAlias,
              elevation: 3,
              color: cover,
              child: Stack(
                children: [
                  // 松紧带(本子的签名元素): 右侧竖向, 深一档的同色偏黑
                  Positioned(
                    top: 0, bottom: 0,
                    right: cardW * 0.12,
                    width: (cardW * 0.035).clamp(5.0, 9.0),
                    child: ColoredBox(
                        color: Colors.black.withValues(alpha: 0.38)),
                  ),
                  // 居中标题
                  Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text('$w × $h',
                            style: TextStyle(
                                fontSize: (cardW * 0.055).clamp(13.0, 17.0),
                                fontWeight: FontWeight.w700,
                                color: Colors.white)),
                        const SizedBox(height: 2),
                        Text('${pages.length} 页',
                            style: TextStyle(
                                fontSize: (cardW * 0.042).clamp(10.0, 12.0),
                                color: Colors.white.withValues(alpha: 0.85))),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ));
      });
      return SingleChildScrollView(
        controller: _gridScroll,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
          child: Wrap(
            alignment: WrapAlignment.start,
            runAlignment: WrapAlignment.start,
            crossAxisAlignment: WrapCrossAlignment.end, // 书架: 底边对齐
            spacing: spacing,
            runSpacing: spacing,
            children: cards,
          ),
        ),
      );
    });
  }

  /// 执行 OCR 搜索: 全库匹配, 结果跨组展示
  Future<void> _runSearch(String q) async {
    q = q.trim();
    if (q.isEmpty) return;
    final ids = await _bridge.ocrSearch(q);
    if (!mounted) return;
    final set = ids.toSet();
    setState(() {
      _searchMode = true;
      _searchText = q;
      _searchFieldVisible = false;
      _filteredPages =
          _groupSorted(_pages).where((p) => set.contains(p.id)).toList();
    });
  }

  void _clearSearch() {
    setState(() {
      _searchMode = false;
      _searchFieldVisible = false;
      _applyPageList();
    });
  }

  /// 打开的笔记本: 顶部返回行 + 该本子的页网格(工具条/多选/懒加载复用)。
  // ── 页面详情(圈选/移动/复制粘贴/删除): 治理动作归面板 ──

  double _detailMarginFor(int screenId) {
    for (final p in _pages) {
      if (p.id == screenId) return p.w > p.h ? p.w * 0.15 : p.h * 0.15;
    }
    return 200;
  }

  Future<void> _openPageDetail(int screenId) async {
    final gen = ++_detailGen;
    final margin = _detailMarginFor(screenId);
    final png = await _bridge.exportPagePngBytes(screenId, margin: margin);
    // 取图 1-2s(debug), 期间用户可能已关详情/打开另一页: 迟到响应若
    // 落地, 位图与 _detailPageId 错位 → 对着 A 页画面粘贴进 B 页
    // (用户实测"在 -2 页粘贴的东西出现在 -1 页"的根因)。
    if (!mounted || gen != _detailGen) return;
    if (png == null || png.isEmpty) {
      _toast('页面为空, 没有可编辑的内容');
      return;
    }
    final codec = await ui.instantiateImageCodec(png);
    final frame = await codec.getNextFrame();
    final img = frame.image;
    if (!mounted || gen != _detailGen) return;
    setState(() {
      _detailPageId = screenId;
      _detailImage = img;
      _detailImgW = img.width.toDouble();
      _detailImgH = img.height.toDouble();
      _detailMargin = margin;
      _selectedStrokes = [];
      _lassoPts.clear();
      _dragStart = null;
    });
  }

  void _closePageDetail() {
    _detailGen++; // 作废在途的取图响应
    setState(() {
      _detailPageId = null;
      _detailImage = null;
      _selectedStrokes = [];
      _lassoPts.clear();
      _dragStart = null;
    });
    unawaited(_loadPages());
  }

  /// 页面矩形在位图内的位置(位图像素): margin × 位图/页面 比例
  Rect _detailPageRect(ui.Image img) {
    PageInfo? page;
    for (final p in _pages) {
      if (p.id == _detailPageId) {
        page = p;
        break;
      }
    }
    final pw = page?.w.toDouble() ?? 0;
    final ph = page?.h.toDouble() ?? 0;
    if (pw <= 0 || ph <= 0 || _detailImgW <= 0) return Rect.zero;
    final kx = _detailImgW / (pw + 2 * _detailMargin);
    return Rect.fromLTWH(_detailMargin * kx, _detailMargin * kx, pw * kx, ph * kx);
  }

  Offset _viewToPage(Offset v) =>
      (v - _detailOffset) / _detailScale - Offset(_detailMargin, _detailMargin);

  Future<void> _finishLasso() async {
    if (_lassoPts.length < 3 || _detailPageId == null) {
      setState(() => _lassoPts.clear());
      return;
    }
    final poly = _lassoPts.map(_viewToPage).map((p) => (p.dx, p.dy)).toList();
    final ids = await _bridge.lassoSelect(_detailPageId!, poly);
    if (!mounted) return;
    setState(() {
      _selectedStrokes = ids;
      _lassoPts.clear();
    });
    if (ids.isEmpty) {
      _toast('圈内没有笔迹');
      unawaited(_reloadDetailImage(_detailPageId!));
    } else {
      _toast('已选 ${ids.length} 笔(蓝色高亮) · 拖拽移动 / 右键复制或删除');
      unawaited(_reloadDetailImage(_detailPageId!, highlight: ids));
    }
  }

  Future<void> _finishDrag() async {
    final start = _dragStart;
    if (start == null || _detailPageId == null || _selectedStrokes.isEmpty) {
      setState(() => _dragStart = null);
      return;
    }
    final d = (_dragNow - start) / _detailScale;
    if (d.distance < 1.0) {
      // 有选中时的原地点击 = 取消选择(否则再也无法重新圈选)
      setState(() {
        _dragStart = null;
        _selectedStrokes = [];
      });
      unawaited(_reloadDetailImage(_detailPageId!));
      return;
    }
    final pageId = _detailPageId!;
    final ids = List<int>.from(_selectedStrokes);
    final ok = await _bridge.moveStrokes(pageId, ids, d.dx, d.dy);
    if (!mounted) return;
    if (ok) {
      setState(() {
        _undoMoves.add((pageId, ids, -d.dx, -d.dy));
        _dragStart = null;
      });
      unawaited(_reloadDetailImage(pageId, highlight: _selectedStrokes));
    } else {
      setState(() => _dragStart = null);
      _toast('移动失败');
    }
  }

  Future<void> _reloadDetailImage(int screenId, {List<int> highlight = const []}) async {
    final gen = _detailGen;
    final png = await _bridge.exportPagePngBytes(screenId,
        highlight: highlight, margin: _detailMargin);
    if (!mounted || png == null || png.isEmpty) return;
    final codec = await ui.instantiateImageCodec(png);
    final frame = await codec.getNextFrame();
    final img = frame.image;
    // 详情已关闭/切到别的页: 这张图作废(代数不对 = 迟到响应)
    if (!mounted || gen != _detailGen || _detailPageId != screenId) return;
    setState(() => _detailImage = img);
  }

  Future<void> _copySelection() async {
    if (_detailPageId == null || _selectedStrokes.isEmpty) return;
    final payload = await _bridge.copyStrokes(_detailPageId!, _selectedStrokes);
    if (!mounted) return;
    _clipPayload = payload;
    _toast(payload.isEmpty ? '没有可复制的内容' : '已复制 ${_selectedStrokes.length} 笔');
  }

  Future<void> _pasteAt(Offset viewPos) async {
    if (_clipPayload.isEmpty || _detailPageId == null) {
      _toast('剪贴板是空的');
      return;
    }
    final p = _viewToPage(viewPos);
    final n = await _bridge.pasteStrokes(_detailPageId!, _clipPayload, p.dx, p.dy);
    if (!mounted) return;
    if (n > 0) {
      _toast('已粘贴 $n 笔');
      unawaited(_reloadDetailImage(_detailPageId!));
    } else {
      _toast('粘贴失败');
    }
  }

  Future<void> _deleteSelection() async {
    if (_detailPageId == null || _selectedStrokes.isEmpty) return;
    final pageId = _detailPageId!;
    final ids = List<int>.from(_selectedStrokes);
    final ok = await _bridge.deleteStrokes(pageId, ids);
    if (!mounted) return;
    if (ok) {
      setState(() => _selectedStrokes = []);
      unawaited(_reloadDetailImage(pageId));
      _toast('已删除 ${ids.length} 笔');
    } else {
      _toast('删除失败');
    }
  }

  Future<void> _undoLastMove() async {
    if (_undoMoves.isEmpty) {
      _toast('没有可撤销的移动');
      return;
    }
    final (pageId, ids, dx, dy) = _undoMoves.removeLast();
    final ok = await _bridge.moveStrokes(pageId, ids, dx, dy);
    if (!mounted) return;
    if (ok && _detailPageId == pageId) {
      unawaited(_reloadDetailImage(pageId));
    }
  }

  void _showDetailMenu(Offset globalPos) {
    final sel = _selectedStrokes.isNotEmpty;
    // showMenu 的 position 相对当前 Overlay。pos 已是全局坐标
    // (TapUpDetails.globalPosition), 只需相对 Overlay 反算; 不要用
    // State 的 context 找 RenderBox(那是整面板, 原点错 → 菜单偏移)。
    final box = context.findRenderObject() as RenderBox;
    final canvasPos = box.globalToLocal(globalPos); // 画布局部(粘贴锚点用)
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox;
    final rel = overlay.globalToLocal(globalPos);
    // RelativeRect.fromLTRB 的 right/bottom = 距 Overlay 右/下边缘的距离
    // (不是坐标!): 给一个以点击点为锚的小矩形, 并夹紧到 Overlay 内缘,
    // 保证菜单完整弹出(此前把坐标当 right 传, 点靠右时矩形退化残缺)。
    const menuW = 220.0, menuH = 170.0;
    final dx = rel.dx
        .clamp(8.0, (overlay.size.width - menuW).clamp(8.0, double.infinity));
    final dy = rel.dy
        .clamp(8.0, (overlay.size.height - menuH).clamp(8.0, double.infinity));
    showMenu<String>(
      context: context,
      position: RelativeRect.fromLTRB(
        dx,
        dy,
        (overlay.size.width - dx - 1).clamp(0.0, double.infinity),
        (overlay.size.height - dy - 1).clamp(0.0, double.infinity),
      ),
      items: [
        if (sel)
          const PopupMenuItem(
              value: 'copy', height: 36,
              child: Row(children: [
                Icon(Icons.copy, size: 16),
                SizedBox(width: 8),
                Text('复制所选', style: TextStyle(fontSize: 13)),
              ])),
        if (_clipPayload.isNotEmpty)
          const PopupMenuItem(
              value: 'paste', height: 36,
              child: Row(children: [
                Icon(Icons.paste, size: 16),
                SizedBox(width: 8),
                Text('粘贴到这里', style: TextStyle(fontSize: 13)),
              ])),
        if (sel)
          const PopupMenuItem(
              value: 'delete', height: 36,
              child: Row(children: [
                Icon(Icons.delete_outline, size: 16, color: Colors.red),
                SizedBox(width: 8),
                Text('删除所选', style: TextStyle(fontSize: 13)),
              ])),
      ],
    ).then((v) {
      if (v == 'copy') unawaited(_copySelection());
      if (v == 'paste') unawaited(_pasteAt(canvasPos));
      if (v == 'delete') unawaited(_deleteSelection());
    });
  }

  Widget _buildPageDetail() {
    final img = _detailImage;
    if (img == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return Column(children: [
      Padding(
          padding: const EdgeInsets.fromLTRB(12, 6, 12, 2),
          child: Row(children: [
            TextButton.icon(
              onPressed: _closePageDetail,
              icon: const Icon(Icons.arrow_back, size: 16),
              label: const Text('本子', style: TextStyle(fontSize: 13)),
            ),
            const SizedBox(width: 8),
            Text('第 $_detailPageId 页 · ${_selectedStrokes.length} 笔已选',
                style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
            const Spacer(),
            TextButton.icon(
              onPressed: _copySelection,
              icon: const Icon(Icons.copy, size: 16),
              label: const Text('复制', style: TextStyle(fontSize: 13)),
            ),
            TextButton.icon(
              onPressed: _deleteSelection,
              icon: const Icon(Icons.delete_outline, size: 16),
              label: const Text('删除所选', style: TextStyle(fontSize: 13)),
            ),
            TextButton.icon(
              onPressed: _undoLastMove,
              icon: const Icon(Icons.undo, size: 16),
              label: const Text('撤销移动', style: TextStyle(fontSize: 13)),
            ),
          ]),
        ),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 12),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text('左键拖圈 = 选择 · 拖拽已选 = 移动 · 右键 = 复制/粘贴 · 点空白 = 取消选择',
                style: TextStyle(fontSize: 11, color: Colors.grey)),
          ),
        ),
        Expanded(
          // fit 在画布层算(与 CustomPaint 同一约束): 此前按整个详情列的
          // 高度算, 工具条+提示行吃掉的部分导致位图溢出被裁(页面只露
          // 顶部一条, 粘贴/移动的内容大量落进裁掉区 = "没反应/N-1 错觉")
          child: LayoutBuilder(builder: (context, cc) {
            final sw2 =
                _detailImgW <= 0 ? 1.0 : cc.maxWidth / _detailImgW;
            final sh2 =
                _detailImgH <= 0 ? 1.0 : cc.maxHeight / _detailImgH;
            _detailScale = sw2 < sh2 ? sw2 : sh2;
            _detailOffset = Offset(
              (cc.maxWidth - _detailImgW * _detailScale) / 2,
              (cc.maxHeight - _detailImgH * _detailScale) / 2,
            );
            return GestureDetector(
            behavior: HitTestBehavior.deferToChild,
            onPanStart: (d) {
              if (_selectedStrokes.isNotEmpty) {
                _dragStart = d.localPosition;
                _dragNow = d.localPosition;
              } else {
                _lassoPts
                  ..clear()
                  ..add(d.localPosition);
              }
            },
            onPanUpdate: (d) {
              if (_dragStart != null) {
                _dragNow = d.localPosition;
              } else if (_lassoPts.isNotEmpty) {
                setState(() => _lassoPts.add(d.localPosition));
              }
            },
            onPanEnd: (_) {
              if (_dragStart != null) {
                unawaited(_finishDrag());
              } else {
                unawaited(_finishLasso());
              }
            },
            onSecondaryTapUp: (d) => _showDetailMenu(d.globalPosition),
            child: CustomPaint(
              painter: _DetailPainter(
                image: img,
                scale: _detailScale,
                offset: _detailOffset,
                lasso: _lassoPts,
                dragFrom: _dragStart,
                dragTo: _dragStart != null ? _dragNow : null,
                pageRect: _detailPageRect(img),
              ),
              child: const SizedBox.expand(),
            ),
            );
          }),
        ),
      ]);
  }

  Widget _buildNotebookPages() {
    final key = _openNotebook!;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 6, 12, 2),
          child: Row(
            children: [
              TextButton.icon(
                onPressed: () => setState(() {
                  _openNotebook = null;
                  _multiSelect = false;
                  _selectedPageIds.clear();
                  _searchMode = false;
                  _searchFieldVisible = false;
                  _applyPageList();
                }),
                icon: const Icon(Icons.arrow_back, size: 16),
                label: const Text('本子', style: TextStyle(fontSize: 13)),
              ),
              const SizedBox(width: 4),
              if (_searchMode) ...[
                Text('搜索 “$_searchText”',
                    style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600)),
                const SizedBox(width: 6),
                IconButton(
                  tooltip: '清除搜索',
                  onPressed: _clearSearch,
                  icon: const Icon(Icons.close, size: 16),
                ),
              ] else ...[
                Text(key.replaceAll('x', ' × '),
                    style: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w600)),
                const SizedBox(width: 8),
              ],
              Text('${_filteredPages.length} 页',
                  style: const TextStyle(fontSize: 12, color: _inkFaint)),
              const Spacer(),
              if (_searchFieldVisible)
                SizedBox(
                  width: 200,
                  height: 30,
                  child: TextField(
                    autofocus: true,
                    style: const TextStyle(fontSize: 13),
                    decoration: const InputDecoration(
                      hintText: '搜索页内文字(OCR)',
                      isDense: true,
                      contentPadding:
                          EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                      border: OutlineInputBorder(),
                    ),
                    onSubmitted: _runSearch,
                  ),
                )
              else
                IconButton(
                  tooltip: 'OCR 搜索',
                  onPressed: () => setState(() => _searchFieldVisible = true),
                  icon: const Icon(Icons.search, size: 18),
                ),
            ],
          ),
        ),
        Expanded(
          child: _filteredPages.isEmpty
              ? Center(
                  child: Text(
                      _searchMode
                          ? '没有匹配的页(该文字未被 OCR 识别?)'
                          : '本子是空的',
                      style:
                          const TextStyle(fontSize: 14, color: Colors.grey)))
              : GridView.builder(
                  controller: _gridScroll,
                  itemCount: _filteredPages.length,
                  padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                  gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
                    maxCrossAxisExtent: _filteredPages.first.w >=
                            _filteredPages.first.h
                        ? 340
                        : 300,
                    mainAxisSpacing: 8,
                    crossAxisSpacing: 8,
                    // cell 比例 = 本子的真实比例(宽页是宽卡, 竖页是高卡)
                    childAspectRatio: _filteredPages.first.w /
                        _filteredPages.first.h,
                  ),
                  itemBuilder: (context, i) {
                    final page = _filteredPages[i];
                    final card = _buildPageCard(page, i);
                    if (_multiSelect || _searchMode) return card;
                    // 长按拖动换位置(iOS 弹簧桌式): 悬停别的卡片时本地
                    // 列表实时移位(其他页让出), 松手一次性提交 DB。
                    // ValueKey 保活手势元素: 列表移位时拖拽不中断。
                    return LongPressDraggable<int>(
                      key: ValueKey('page-${page.id}'),
                      data: page.id,
                      delay: const Duration(milliseconds: 300),
                      onDragStarted: () {
                        _dragOrderBackup = List<PageInfo>.from(_filteredPages);
                        _draggingPageId = page.id;
                      },
                      onDragEnd: (_) => unawaited(_commitPageDrag()),
                      feedback: Material(
                        elevation: 8,
                        borderRadius: BorderRadius.circular(10),
                        child: SizedBox(
                            width: 260, child: _buildPageCard(page, i)),
                      ),
                      childWhenDragging: Opacity(
                          opacity: 0.35, child: card),
                      child: DragTarget<int>(
                        onWillAcceptWithDetails: (d) => d.data != page.id,
                        onMove: (d) => _livePageShift(d.data),
                        onAcceptWithDetails: (d) => _livePageShift(d.data),
                        builder: (context, _, _) => card,
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }


  /// 前后移小按钮(活页本卡片右上角): 微动画零心智负担, 移动动作归面板。
  /// 拖拽中: 把拖动的页移到目标页位置, 其余页实时让出(纯本地列表,
  /// 松手才提交 DB)。拖动项保留在列表内(只换位), 配合 ValueKey 元素
  /// 匹配, 拖拽手势跨移位存活。
  void _livePageShift(int targetPageId) {
    final dragged = _draggingPageId;
    if (dragged == null || dragged == targetPageId) return;
    final from = _filteredPages.indexWhere((p) => p.id == dragged);
    final to = _filteredPages.indexWhere((p) => p.id == targetPageId);
    if (from < 0 || to < 0 || from == to) return;
    setState(() {
      final item = _filteredPages.removeAt(from);
      _filteredPages.insert(to, item);
    });
  }

  /// 拖拽结束: 顺序变了才提交 —— 锚点 = 落点处相邻页(有后邻就"移到它
  /// 之前", 否则"移到前邻之后"), 一次 reorder_page 落库; 失败回滚本地。
  Future<void> _commitPageDrag() async {
    final dragged = _draggingPageId;
    _draggingPageId = null;
    if (dragged == null) return;
    final backupIds = _dragOrderBackup.map((p) => p.id).toList();
    final nowIds = _filteredPages.map((p) => p.id).toList();
    if (backupIds.length == nowIds.length && 
        List.generate(nowIds.length, (i) => nowIds[i] == backupIds[i]).every((v) => v)) {
      return; // 没挪窝(拿起又放回)
    }
    final idx = _filteredPages.indexWhere((p) => p.id == dragged);
    if (idx < 0 || _filteredPages.length < 2) return;
    int anchorId;
    bool before;
    if (idx + 1 < _filteredPages.length) {
      anchorId = _filteredPages[idx + 1].id;
      before = true;
    } else {
      anchorId = _filteredPages[idx - 1].id;
      before = false;
    }
    final ok = await _bridge.reorderPage(dragged, anchorId, before: before);
    if (!mounted) return;
    if (ok) {
      unawaited(_loadPages()); // 以 DB 为准刷新(缩略图缓存未失效, 很快)
    } else {
      setState(() {
        _filteredPages = List<PageInfo>.from(_dragOrderBackup);
      });
      _toast('移动失败, 已还原');
    }
  }


  /// 活页本网格工具条:常态 = 页数 + 「多选」入口;
  /// 多选态 = 已选计数 + 全选 / 删除所选 / 取消。
  Widget _buildGridToolbar() {
    const faint = TextStyle(fontSize: 12, color: _inkFaint);
    if (!_multiSelect) {
      return Row(children: [
        Text('${_filteredPages.length} 页', style: faint),
        const Spacer(),
        TextButton.icon(
          onPressed: () => setState(() {
            _multiSelect = true;
            _selectedPageIds.clear();
          }),
          icon: const Icon(Icons.checklist, size: 16),
          label: const Text('多选', style: TextStyle(fontSize: 13)),
        ),
      ]);
    }
    return Row(children: [
      Text('已选 ${_selectedPageIds.length} 页', style: faint),
      const Spacer(),
      TextButton.icon(
        onPressed: (_batchDeleting || _selectedPageIds.isEmpty) ? null : () async {
          setState(() => _batchDeleting = true);
          try {
            final ok = await _bridge.exportSelectedPdf(_selectedPageIds.toList());
            if (mounted) {
              _toast(ok ? '已导出 ${_selectedPageIds.length} 页 PDF 到桌面' : '导出失败');
            }
          } finally {
            if (mounted) setState(() => _batchDeleting = false);
          }
        },
        style: TextButton.styleFrom(foregroundColor: _ink),
        icon: const Icon(Icons.picture_as_pdf, size: 16),
        label: const Text('导出 PDF', style: TextStyle(fontSize: 13)),
      ),
      TextButton(
        onPressed: () => setState(() {
          if (_selectedPageIds.length == _filteredPages.length) {
            _selectedPageIds.clear();
          } else {
            _selectedPageIds.addAll(_filteredPages.map((p) => p.id));
          }
        }),
        child: Text(_selectedPageIds.length == _filteredPages.length && _filteredPages.isNotEmpty
            ? '取消全选' : '全选', style: const TextStyle(fontSize: 13)),
      ),
      TextButton(
        onPressed: (_batchDeleting || _selectedPageIds.isEmpty) ? null : _confirmBatchDelete,
        style: TextButton.styleFrom(foregroundColor: Colors.red),
        child: Text(_batchDeleting ? '删除中…' : '删除所选(${_selectedPageIds.length})',
            style: const TextStyle(fontSize: 13)),
      ),
      TextButton(
        onPressed: () => setState(() {
          _multiSelect = false;
          _selectedPageIds.clear();
        }),
        child: const Text('取消', style: TextStyle(fontSize: 13)),
      ),
    ]);
  }

  /// 批量删除确认 + 逐页调用既有删除通道;完成后重载页面列表。
  Future<void> _confirmBatchDelete() async {
    final ids = _selectedPageIds.toSet();
    if (ids.isEmpty) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('批量删除'),
        content: Text('确定删除所选 ${ids.length} 页及其所有笔迹吗?'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('取消')),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: TextButton.styleFrom(foregroundColor: Colors.red),
            child: Text('删除 $ids.length 页'.replaceFirst(r'$ids.length', '${ids.length}')),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() => _batchDeleting = true);
    int ok = 0;
    for (final id in ids) {
      try {
        if (await _bridge.deletePage(id)) {
          ok++;
          _thumbnailCache.remove(id);
          _pages.removeWhere((p) => p.id == id);
          _filteredPages.removeWhere((p) => p.id == id);
        }
      } catch (e) {
        debugPrint('[Pages] delete $id failed: $e');
      }
    }
    if (!mounted) return;
    setState(() {
      _multiSelect = false;
      _selectedPageIds.clear();
      _batchDeleting = false;
    });
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('已删除 $ok 页'), duration: const Duration(seconds: 2)),
    );
    unawaited(_loadPages()); // 重载列表,页码/缩略图与服务端状态对齐
  }

  /// 页列表装配: 未开本子 = 全部页按组排序(本子列表的顺序);
  /// 已开本子 = 只保留该组的页。删页/刷新统一走这里。
  void _applyPageList() {
    if (_openNotebook == null) {
      _filteredPages = _groupSorted(_pages);
      return;
    }
    _filteredPages = _groupSorted(_pages)
        .where((p) => '${p.w}x${p.h}' == _openNotebook)
        .toList();
  }

  /// 笔记本(分辨率组)列表: key "WxH" → 该组的页(组内 id 升序)。
  /// 顺序与 _groupSorted 一致: 最近活跃的组在前。
  Map<String, List<PageInfo>> get _notebooks {
    final groups = <String, List<PageInfo>>{};
    for (final p in _pages) {
      groups.putIfAbsent('${p.w}x${p.h}', () => []).add(p);
    }
    // 组内保持 _pages 传入顺序(order_index);组的先后仍按最近活跃
    final keys = groups.keys.toList()
      ..sort((a, b) {
        int mx(String k) =>
            groups[k]!.map((p) => p.id).reduce((x, y) => x > y ? x : y);
        return mx(b).compareTo(mx(a));
      });
    return {for (final k in keys) k: groups[k]!};
  }

  /// 页按分辨率分组(页 = 对应尺寸玻璃的快照): 最近活跃的组排最前,
  /// 组内按页 id(时间)序。当前屏几何几乎总是最新组 → 开箱即当前本子。
  List<PageInfo> _groupSorted(List<PageInfo> pages) {
    if (pages.isEmpty) return pages;
    final groups = <String, List<PageInfo>>{};
    for (final p in pages) {
      groups.putIfAbsent('${p.w}x${p.h}', () => []).add(p);
    }
    final keys = groups.keys.toList()
      ..sort((a, b) {
        int mx(String k) => groups[k]!.map((p) => p.id).reduce((x, y) => x > y ? x : y);
        return mx(b).compareTo(mx(a)); // 最近活跃的组在前
      });
    final out = <PageInfo>[];
    for (final k in keys) {
      // 组内保持传入顺序(list_screens 已按 order_index 排)——此前按 id
      // 重排把页重排的结果整个吞掉(按钮"按了没反应"的根因)
      out.addAll(groups[k]!);
    }
    return out;
  }

  /// 该页是否属于"第一个(最近活跃)组" —— 异组卡片显示分辨率徽标。
  bool _isPrimaryGroup(PageInfo page) {
    if (_filteredPages.isEmpty) return true;
    final first = _filteredPages.first;
    return page.w == first.w && page.h == first.h;
  }

  Widget _buildPageCard(PageInfo page, int index) {
    if (page.thumbnail == null && _thumbnailCache.containsKey(page.id)) {
      page.thumbnail = _thumbnailCache[page.id];
    }
    final selected = _selectedPageIds.contains(page.id);

    return GestureDetector(
      // 多选态:右键删除菜单禁用(统一走批量删除)
      onSecondaryTapUp: _multiSelect
          ? null
          : (details) {
              final overlay =
                  Overlay.of(context).context.findRenderObject() as RenderBox;
              showMenu<String>(
                context: context,
                position: RelativeRect.fromLTRB(
                  details.globalPosition.dx,
                  details.globalPosition.dy,
                  overlay.size.width - details.globalPosition.dx,
                  overlay.size.height - details.globalPosition.dy,
                ),
                items: [
                  const PopupMenuItem(
                      value: 'detail', height: 36,
                      child: Row(children: [
                        Icon(Icons.draw_outlined, size: 16),
                        SizedBox(width: 8),
                        Text('编辑内容(圈选/移动/复制)', style: TextStyle(fontSize: 13)),
                      ])),
                  const PopupMenuItem(
                      value: 'png', height: 36,
                      child: Row(children: [
                        Icon(Icons.image_outlined, size: 16),
                        SizedBox(width: 8),
                        Text('导出此页 PNG', style: TextStyle(fontSize: 13)),
                      ])),
                  const PopupMenuItem(
                      value: 'svg', height: 36,
                      child: Row(children: [
                        Icon(Icons.polyline_outlined, size: 16),
                        SizedBox(width: 8),
                        Text('导出此页 SVG', style: TextStyle(fontSize: 13)),
                      ])),
                  const PopupMenuItem(
                      value: 'info', height: 36,
                      child: Row(children: [
                        Icon(Icons.info_outline, size: 16),
                        SizedBox(width: 8),
                        Text('复制页信息', style: TextStyle(fontSize: 13)),
                      ])),
                  const PopupMenuDivider(),
                  const PopupMenuItem(
                      value: 'delete', height: 36,
                      child: Row(children: [
                        Icon(Icons.delete_outline, size: 16, color: Colors.red),
                        SizedBox(width: 8),
                        Text('删除此页面', style: TextStyle(fontSize: 13)),
                      ])),
                ],
              ).then((v) async {
                if (v == 'detail') {
                  await _openPageDetail(page.id);
                } else if (v == 'delete') {
                  _confirmDeletePage(page);
                } else if (v == 'png') {
                  final ok = await _bridge.exportPagePng(page.id);
                  _toast(ok ? '已导出 PNG 到桌面' : '导出失败(页为空?)');
                } else if (v == 'svg') {
                  final ok = await _bridge.exportPageSvg(page.id);
                  _toast(ok ? '已导出 SVG 到桌面' : '导出失败(页为空?)');
                } else if (v == 'info') {
                  final t = DateTime.fromMillisecondsSinceEpoch(page.id * 1000);
                  Clipboard.setData(ClipboardData(text:
                      '页 ${page.id}\n尺寸 ${page.w}×${page.h}\n笔迹 ${page.strokeCount}\n'
                      '创建 ${t.year}-${t.month.toString().padLeft(2, '0')}-${t.day.toString().padLeft(2, '0')}'));
                  _toast('页信息已复制');
                }
              });
            },
      child: Card(
        clipBehavior: Clip.antiAlias,
        shape: _multiSelect && selected
            ? RoundedRectangleBorder(
                side: const BorderSide(color: _penRed, width: 2),
                borderRadius: BorderRadius.circular(10))
            : null,
        child: InkWell(
          onTap: () {
            if (_multiSelect) {
              setState(() => selected
                  ? _selectedPageIds.remove(page.id)
                  : _selectedPageIds.add(page.id));
              return;
            }
            _bridge.navigateToPage(page.id);
          },
          child: Stack(
            children: [
              if (!_isPrimaryGroup(page))
                Positioned(
                  top: 4, left: 4,
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                    decoration: BoxDecoration(
                      color: Colors.black.withValues(alpha: 0.55),
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text('${page.w}×${page.h}',
                        style: const TextStyle(fontSize: 9, color: Colors.white)),
                  ),
                ),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Thumbnail(格子比例 = 页比例, 填满即可)
                  Expanded(
                    child: page.thumbnail != null
                        ? Image.memory(page.thumbnail!,
                            fit: BoxFit.cover, gaplessPlayback: true)
                        : const _ThumbSkeleton(),
                  ),
                  // Page info
                  Padding(
                    padding: const EdgeInsets.fromLTRB(8, 6, 4, 6),
                    child: Text('页面 ${page.id}',
                        style: const TextStyle(
                            fontWeight: FontWeight.bold, fontSize: 13)),
                  ),
                ],
              ),
              // 多选角标
              if (_multiSelect)
                Positioned(
                  top: 6,
                  right: 6,
                  child: Icon(
                    selected ? Icons.check_circle : Icons.radio_button_unchecked,
                    size: 22,
                    color: selected ? _penRed : Colors.white,
                    shadows: const [Shadow(blurRadius: 4, color: Colors.black38)],
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }

  void _confirmDeletePage(PageInfo page) {
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

  Future<void> _deletePage(PageInfo page) async {
    try {
      final ok = await _bridge.deletePage(page.id);
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

  /// 取当前版本号(桥不可用时静默失败, 版本栏保持 —)。
  Future<void> _loadAppVersion() async {
    try {
      final v = await _bridge.appVersion();
      if (mounted && v.isNotEmpty) setState(() => _appVersion = v);
    } catch (e) {
      debugPrint('[About] appVersion failed: $e');
    }
  }

  /// 手动「检查更新」:一次 GitHub API 往返, 成功/失败都放进 _updateResult。
  Future<void> _checkUpdate() async {
    if (_checkingUpdate) return;
    setState(() {
      _checkingUpdate = true;
      _updateResult = null;
    });
    Map<dynamic, dynamic> result;
    try {
      result = await _bridge.checkUpdate();
      // 空 map = 管道没连上(Windows),或对端没实现该消息
      if (result.isEmpty) {
        result = {'ok': false, 'error': '未连接到主程序'};
      }
    } catch (e) {
      result = {'ok': false, 'error': '$e'};
    }
    if (!mounted) return;
    setState(() {
      _checkingUpdate = false;
      _updateResult = result;
    });
  }

  /// 「关于」区:当前版本 + 检查更新(结果:已是最新 / 新版本+下载页 / 失败原因)。
  /// 「立即更新」第一步:确认对话框(版本 + release notes + 提示会重启)。
  Future<bool> _confirmUpdate() async {
    final r = _updateResult!;
    final tag = (r['latest'] as String?) ?? '';
    final notes = (r['notes'] as String?) ?? '';
    final res = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('更新到 $tag ?'),
        content: SizedBox(
          width: 440,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                const Text(
                  '将下载新版安装包、退出并自动重启玻璃涂鸦。\n笔迹已实时存盘,不会丢失。',
                  style: TextStyle(fontSize: 13),
                ),
                if (notes.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  const Text('更新内容:',
                      style: TextStyle(
                          fontSize: 13, fontWeight: FontWeight.bold)),
                  const SizedBox(height: 6),
                  Text(notes,
                      style: const TextStyle(
                          fontSize: 12.5, color: _inkFaint, height: 1.5)),
                ],
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('取消')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: const Text('下载并更新')),
        ],
      ),
    );
    return res ?? false;
  }

  /// 「立即更新」:确认 → 下载(进度)→ 解包 → 等待用户点「立即重启更新」。
  Future<void> _startUpdate() async {
    if (_upd != _UpdPhase.idle || _updateResult == null) return;
    if (!await _confirmUpdate() || !mounted) return;
    final r = _updateResult!;
    setState(() {
      _upd = _UpdPhase.downloading;
      _updTag = (r['latest'] as String?) ?? '';
      _updReceived = 0;
      _updTotal = 0;
      _updError = '';
    });
    _updSub = _bridge.downloadUpdate().listen(
      _onUpdateFrame,
      onError: (Object e) {
        if (!mounted) return;
        setState(() {
          _upd = _UpdPhase.failed;
          _updError = '下载失败:$e';
        });
      },
      onDone: () => _updSub = null,
    );
  }

  /// 下载流的每一帧;`done` 帧决定去解包还是报错。
  void _onUpdateFrame(Map<dynamic, dynamic> f) {
    if (!mounted) return;
    if (f['done'] != true) {
      setState(() {
        _updReceived = (f['received'] as num?)?.toInt() ?? 0;
        _updTotal = (f['total'] as num?)?.toInt() ?? 0;
      });
      return;
    }
    _updSub?.cancel();
    _updSub = null;
    final err = (f['error'] as String?) ?? '';
    if (err.isNotEmpty) {
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = err;
      });
      return;
    }
    _stageUpdate();
  }

  /// 下载完成 → 解包 DMG(dmg 路径 Rust 侧自己找, 不经 UI 传参)。
  Future<void> _stageUpdate() async {
    setState(() => _upd = _UpdPhase.staging);
    try {
      final s = await _bridge.stageUpdate(_updTag);
      if (!mounted) return;
      setState(() {
        if (s['ok'] == true) {
          _upd = _UpdPhase.ready;
        } else {
          _upd = _UpdPhase.failed;
          _updError = (s['message'] as String?) ?? '解包失败';
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = '解包失败:$e';
      });
    }
  }

  /// 「立即重启更新」:拉起帮手 → 本进程退出(正常情况下 Future 永不返回)。
  Future<void> _applyUpdate() async {
    setState(() => _upd = _UpdPhase.applying);
    try {
      final a = await _bridge.applyUpdate();
      // 走到这里 = 退出失败(成功的路径进程已经没了)
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = (a['message'] as String?) ?? '退出更新失败';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = '退出更新失败:$e';
      });
    }
  }

  /// 取消下载(解包后的 ready 状态不给取消 —— 文件已就绪,重启才生效)。
  void _cancelUpdate() {
    _updSub?.cancel();
    _updSub = null;
    setState(() => _upd = _UpdPhase.idle);
  }

  static String _fmtMb(int bytes) =>
      '${(bytes / 1048576).toStringAsFixed(1)} MB';

  /// 「立即更新」进行中的 UI(下载进度 / 解包中 / 待重启 / 失败)。
  Widget _buildUpdateProgress(TextStyle faint) {
    switch (_upd) {
      case _UpdPhase.downloading:
        final value = _updTotal > 0 ? _updReceived / _updTotal : null;
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          LinearProgressIndicator(value: value),
          const SizedBox(height: 8),
          Row(children: [
            Expanded(
              child: Text(
                _updTotal > 0
                    ? '下载中 ${_fmtMb(_updReceived)} / ${_fmtMb(_updTotal)}'
                    : '下载中…',
                style: faint,
              ),
            ),
            TextButton(
                onPressed: _cancelUpdate,
                child: const Text('取消',
                    style: TextStyle(fontSize: 13))),
          ]),
        ]);
      case _UpdPhase.staging:
        return Row(children: [
          const SizedBox(
              width: 14, height: 14,
              child: CircularProgressIndicator(strokeWidth: 2)),
          const SizedBox(width: 8),
          Text('下载完成,正在解包校验…', style: faint),
        ]);
      case _UpdPhase.ready:
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Text('已下载并解包完成。重启后生效。',
              style: TextStyle(fontSize: 13, color: _ink)),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: _applyUpdate,
            child: const Text('立即重启更新'),
          ),
        ]);
      case _UpdPhase.applying:
        return Row(children: [
          const SizedBox(
              width: 14, height: 14,
              child: CircularProgressIndicator(strokeWidth: 2)),
          const SizedBox(width: 8),
          Text('正在退出并重启…', style: faint),
        ]);
      case _UpdPhase.failed:
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('更新失败:$_updError',
              style: const TextStyle(fontSize: 13, color: _penRed)),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: () => setState(() => _upd = _UpdPhase.idle),
            child: const Text('重试'),
          ),
        ]);
      case _UpdPhase.idle:
        return const SizedBox.shrink();
    }
  }

  Widget _buildAboutSection() {
    const faint = TextStyle(fontSize: 13, color: _inkFaint);

    final Widget status;
    if (_upd != _UpdPhase.idle) {
      // 点过「立即更新」之后,状态区整体交给更新状态机
      status = _buildUpdateProgress(faint);
    } else if (_checkingUpdate) {
      status = Row(children: [
        const SizedBox(
            width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2)),
        const SizedBox(width: 8),
        const Text('正在检查…', style: faint),
      ]);
    } else if (_updateResult != null) {
      final r = _updateResult!;
      if (r['ok'] == true && r['hasUpdate'] == true) {
        final url = (r['url'] as String?) ?? '';
        status = Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('发现新版本 ${r['latest']}',
              style: const TextStyle(
                  fontSize: 14, fontWeight: FontWeight.bold, color: _penRed)),
          const SizedBox(height: 8),
          Wrap(spacing: 8, runSpacing: 6, children: [
            OutlinedButton(
              onPressed: _startUpdate,
              child: const Text('立即更新'),
            ),
            OutlinedButton.icon(
              icon: const Icon(Icons.open_in_new, size: 15),
              label: const Text('打开下载页'),
              onPressed: () async {
                try {
                  await _bridge.openUrl(url.isEmpty ? _releasesPage : url);
                } catch (e) {
                  debugPrint('[About] openUrl failed: $e');
                }
              },
            ),
          ]),
        ]);
      } else if (r['ok'] == true) {
        status = Text('已是最新版本 (v${r['current']})', style: faint);
      } else {
        status = Text('检查失败:${r['error'] ?? '未知错误'}',
            style: const TextStyle(fontSize: 13, color: _penRed));
      }
    } else {
      status = const Text('点一下向 GitHub 查询最新正式版(需联网)',
          style: TextStyle(fontSize: 12, color: _inkFaint));
    }

    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Row(children: [
        const Text('当前版本', style: TextStyle(fontSize: 14, color: _ink)),
        const Spacer(),
        Text(_appVersion == null ? '—' : 'v$_appVersion',
            style: faint.copyWith(fontSize: 14)),
      ]),
      const SizedBox(height: 10),
      OutlinedButton(
        onPressed: _checkingUpdate ? null : _checkUpdate,
        child: const Text('检查更新'),
      ),
      const SizedBox(height: 10),
      status,
    ]);
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
        row([key('G', '导出图片'), key('`', '上一页'), key('1', '下一页')], indent: 22),
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
        if (Platform.isMacOS) ...[
          Row(
            children: [
              Text('翻页动效',
                  style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
              const Spacer(),
              SegmentedButton<int>(
                showSelectedIcon: false,
                style: const ButtonStyle(
                  visualDensity: VisualDensity(horizontal: -2, vertical: -2),
                ),
                segments: const [
                  ButtonSegment(value: 0, label: Text('系统动效')),
                  ButtonSegment(value: 1, label: Text('时光隧道')),
                ],
                selected: {_flipEffect},
                onSelectionChanged: (sel) {
                  setState(() => _flipEffect = sel.first);
                  _setSetting('flipEffect', sel.first);
                },
              ),
            ],
          ),
          const Divider(height: 24),
        ],
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
          title: const Text('自由涂鸦模式', style: TextStyle(fontSize: 15)),
          subtitle: const Text('显示「自由涂鸦」画布:不翻页,镜头自由移动',
              style: TextStyle(fontSize: 12)),
          value: _showFreeCanvas,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() {
              _showFreeCanvas = v;
              _syncTabCount();
              if (!v && _infiniteCanvas) {
                // tab 移除后只剩活页本:模式同步回翻页
                _infiniteCanvas = false;
                _setSetting('infiniteCanvas', false);
              }
            });
            _setSetting('showFreeCanvas', v);
          },
        )),
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
        if (_showGrid)
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
        if (_showGrid)
          Padding(
            padding: const EdgeInsets.only(left: 16, right: 16, bottom: 4),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('分栏', style: TextStyle(fontSize: 13, color: _inkFaint)),
                const SizedBox(height: 4),
                SizedBox(
                  width: double.infinity,
                  child: SegmentedButton<int>(
                    showSelectedIcon: false,
                    style: const ButtonStyle(
                      visualDensity: VisualDensity(horizontal: -2, vertical: -2),
                    ),
                    segments: const [
                      ButtonSegment(value: 0, label: Text('无')),
                      ButtonSegment(value: 1, label: Text('左右两栏')),
                      ButtonSegment(value: 2, label: Text('上下两栏')),
                      ButtonSegment(value: 3, label: Text('九宫格')),
                    ],
                    selected: {_gridDividerValue},
                    onSelectionChanged: (s) {
                      setState(() => _gridDividerValue = s.first);
                      _setSetting('gridDivider', s.first);
                    },
                  ),
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
          title: const Text('磨砂玻璃跟随涂鸦', style: TextStyle(fontSize: 15)),
          subtitle: const Text('开启后磨砂玻璃随涂鸦一起受 ⌘⌃X 控制；关闭则常驻', style: TextStyle(fontSize: 12)),
          value: _glassFollowStrokes,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _glassFollowStrokes = v);
            _setSetting('glassFollowStrokes', v);
          },
        )),
        _tile(SwitchListTile(
          title: const Text('笔迹描边', style: TextStyle(fontSize: 15)),
          subtitle: const Text('黑白相间虚线描边，任意背景恒可见 · 已持久化', style: TextStyle(fontSize: 12)),
          value: _outlineEnabled,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _outlineEnabled = v);
            _setSetting('outline', v);
          },
        )),
        _tile(SwitchListTile(
          title: const Text('软阴影', style: TextStyle(fontSize: 15)),
          subtitle: const Text('笔迹下方柔和黑影，同色背景保底可见 · 可与描边叠加', style: TextStyle(fontSize: 12)),
          value: _softShadow,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _softShadow = v);
            _setSetting('softShadow', v);
          },
        )),
        _tile(SwitchListTile(
          title: const Text('反色突出', style: TextStyle(fontSize: 15)),
          subtitle: const Text('笔迹逐像素取背景反色 · 实验:视频背景持续追踪，建议关闭磨砂玻璃', style: TextStyle(fontSize: 12)),
          value: _invertInk,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _invertInk = v);
            _setSetting('invertInk', v);
          },
        )),
        _tile(SegmentedButton<int>(
          segments: const [
            ButtonSegment(value: 10, label: Text('10Hz')),
            ButtonSegment(value: 30, label: Text('30Hz')),
            ButtonSegment(value: 60, label: Text('60Hz')),
            ButtonSegment(value: 100, label: Text('100Hz')),
          ],
          selected: {_invertFps},
          showSelectedIcon: false,
          onSelectionChanged: (s2) {
            setState(() => _invertFps = s2.first);
            _setSetting('invertFps', s2.first);
          },
        )),
        _tile(const Padding(
          padding: EdgeInsets.only(left: 4, bottom: 8),
          child: Text('反色追踪帧率上限 · 静态背景不送帧零开销', style: TextStyle(fontSize: 11, color: Colors.grey)),
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

  bool _backupBusy = false;

  Widget _buildBackupButtons() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          '备份是桌面上的单个 .db 文件(一致快照), 换机时可直接替换 glaspen2.db; '
          '恢复是合并方式, 不会删除备份之后新画的内容。',
          style: TextStyle(fontSize: 12, color: Colors.grey),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            FilledButton.icon(
              icon: _backupBusy
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(
                          strokeWidth: 2, color: Colors.white),
                    )
                  : const Icon(Icons.save_alt, size: 18),
              label: Text(_backupBusy ? '处理中…' : '备份全部数据'),
              onPressed: _backupBusy ? null : _backupNow,
            ),
            const SizedBox(width: 8),
            OutlinedButton.icon(
              icon: const Icon(Icons.settings_backup_restore, size: 18),
              label: const Text('从最新备份恢复'),
              onPressed: _backupBusy ? null : _confirmRestore,
            ),
          ],
        ),
      ],
    );
  }

  void _toast(String text) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(text), duration: const Duration(seconds: 4)),
    );
  }

  Future<void> _backupNow() async {
    setState(() => _backupBusy = true);
    try {
      final (ok, message) = await _bridge.backupNow();
      if (!mounted) return;
      _toast(ok ? '已备份到 $message' : '备份失败: $message');
    } catch (e) {
      if (mounted) _toast('备份失败: $e');
    } finally {
      if (mounted) setState(() => _backupBusy = false);
    }
  }

  Future<void> _confirmRestore() async {
    final yes = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('从最新备份恢复'),
        content: const Text(
            '将把桌面上最新的 glaspen2_backup_*.db 合并进当前数据: '
            '同名的页/笔迹以备份为准, 备份之后新画的内容会保留。'),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('取消')),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('恢复'),
          ),
        ],
      ),
    );
    if (yes != true || !mounted) return;

    setState(() => _backupBusy = true);
    try {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('从最新备份恢复', style: TextStyle(fontSize: 16)),
          content: const Text('备份中的页将合并进当前库(不会删除备份之后\n新画的内容)。继续吗?', style: TextStyle(fontSize: 13)),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(ctx, false),
                child: const Text('取消')),
            TextButton(
                onPressed: () => Navigator.pop(ctx, true),
                child: const Text('恢复')),
          ],
        ),
      );
      if (confirmed != true || !mounted) return;
      final (ok, message) = await _bridge.restoreLatestBackup();
      if (!mounted) return;
      _toast(ok ? '已从备份恢复: $message(重启后生效)' : '恢复失败: $message');
      if (ok) {
        // 缩略图缓存与列表都可能过期, 清掉重拉
        _thumbnailCache.clear();
        _pages = [];
        _filteredPages = [];
        _loadPages();
      }
    } catch (e) {
      if (mounted) _toast('恢复失败: $e');
    } finally {
      if (mounted) setState(() => _backupBusy = false);
    }
  }

  bool _pdfExporting = false;

  Future<void> _exportPdf() async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('导出全部页面', style: TextStyle(fontSize: 16)),
        content: const Text('全部页将合成一个 PDF 保存到桌面。',
            style: TextStyle(fontSize: 13)),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: const Text('取消')),
          TextButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: const Text('导出')),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    setState(() => _pdfExporting = true);
    try {
      final ok = await _bridge.exportPdf();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(ok ? 'PDF 已保存到桌面' : '导出失败'),
            duration: const Duration(seconds: 2),
          ),
        );
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


/// 页面详情画布: 位图(fit) + 圈选路径 + 拖拽幽灵(选区包围盒位移)
class _DetailPainter extends CustomPainter {
  _DetailPainter({
    required this.image,
    required this.scale,
    required this.offset,
    required this.lasso,
    required this.dragFrom,
    required this.dragTo,
    required this.pageRect,
  });

  final ui.Image image;
  final double scale;
  final Offset offset;
  final List<Offset> lasso;
  final Offset? dragFrom;
  final Offset? dragTo;
  final Rect pageRect; // 页面矩形(位图像素): 白底 + 页框

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(Offset.zero & size, Paint()..color = const Color(0xFFF2F1EC));
    final dst = Offset.zero & Size(image.width * scale, image.height * scale);
    // 页底白 + 页框(位图透明底): 页外内容落在灰底上, 一眼区分页内外
    if (!pageRect.isEmpty) {
      canvas.drawRect(pageRect, Paint()..color = const Color(0xFFFFFFFF));
      canvas.drawRect(
        pageRect,
        Paint()
          ..color = const Color(0x33000000)
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.0,
      );
    }
    canvas.drawImageRect(
      image,
      Rect.fromLTWH(0, 0, image.width.toDouble(), image.height.toDouble()),
      dst.shift(offset),
      Paint()..filterQuality = FilterQuality.medium,
    );
    if (dragFrom != null && dragTo != null) {
      final d = dragTo! - dragFrom!;
      final rect = dst.shift(offset).inflate(2).translate(d.dx, d.dy);
      canvas.drawRect(
        rect,
        Paint()
          ..color = Colors.blue
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5,
      );
    }
    if (lasso.length >= 2) {
      final path = Path()..moveTo(lasso.first.dx, lasso.first.dy);
      for (final p in lasso.skip(1)) {
        path.lineTo(p.dx, p.dy);
      }
      canvas.drawPath(
        path,
        Paint()
          ..color = const Color(0xCC1A73E8)
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.6,
      );
      canvas.drawPath(
        path,
        Paint()..color = const Color(0x221A73E8)..style = PaintingStyle.fill,
      );
    }
  }

  @override
  bool shouldRepaint(_DetailPainter old) =>
      old.image != image ||
      old.scale != scale ||
      old.offset != offset ||
      old.dragFrom != dragFrom ||
      old.dragTo != dragTo ||
      old.pageRect != pageRect ||
      old.lasso.length != lasso.length;
}
