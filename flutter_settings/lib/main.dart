import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart' show Clipboard, ClipboardData, rootBundle;
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
part 'main.notebook_data.dart';
part 'main.notebook.dart';
part 'main.update.dart';
part 'main.settings_widgets.dart';

void main() {
  // FRB 需要绑定初始化(消息端口/isolate)后才能解析 Rust 符号
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const GlaspenSettingsApp());
}

/// 面板语言。默认中文, **不跟随系统自动检测** —— 由设置里的选择器决定。
/// 设置加载/原生菜单切换时更新这个 notifier, MaterialApp 立即重建。
final ValueNotifier<Locale> appLocale = ValueNotifier(const Locale('zh'));

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
    if (locale != null) return _build(context, locale!);
    return ValueListenableBuilder<Locale>(
      valueListenable: appLocale,
      builder: (context, l, _) => _build(context, l),
    );
  }

  Widget _build(BuildContext context, Locale locale) {
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
  String _language = 'zh'; // 界面语言: zh | en(不再跟随系统)
  bool _pressureMonitor = false;
  bool _launchAtLogin = false;
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
  bool _showBubbleTab = false;
  bool _bubblesLoading = false;
  List<Map<String, dynamic>> _bubbleMsgs = const [];
  final ScrollController _bubbleScroll = ScrollController();
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
  bool _notebookGlass = true; // 活页本外观: true=拟物玻璃 false=笔记纸
  ui.Image? _tabBgImage; // 插画位图(真穿透时铺在网格底层, 卡片挖洞)
  bool _panelTransparent = false; // 真穿透(实验): 面板窗口可透明, 卡片窟窿透出桌面
  /// OCR 搜索(本子页视图内): 激活后网格 = 全库匹配页(跨组)
  bool _searchMode = false;
  bool _searchFieldVisible = false;
  String _searchText = '';
  int _flipEffect = 0; // 翻页动效: 0=macOS 系统 1=时光隧道
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
    _bubbleScroll.dispose();
    _reloadTimer?.cancel();
    _updSub?.cancel(); // 还在下载时销毁面板 = 取消下载
    _chatApiBase.dispose();
    _chatUser.dispose();
    _chatPassword.dispose();
    _bridge.dispose();
    super.dispose();
  }

  void _setLanguage(String code) {
    final lang = code == 'en' ? 'en' : 'zh';
    setState(() => _language = lang);
    _applyLanguage(lang);
    _setSetting('language', lang);
  }

  /// 只改 UI 语言, 不写库(加载/原生菜单推送时用)。
  void _applyLanguage(String code) {
    final lang = code == 'en' ? 'en' : 'zh';
    if (_language != lang) _language = lang;
    final loc = Locale(lang);
    if (appLocale.value != loc) appLocale.value = loc;
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
    // 气泡消息 tab: 位置在自由涂鸦之后(自由涂鸦隐藏时占 index 2)
    final bubbleIdx = 2 + (_showFreeCanvas ? 1 : 0);
    if (_showBubbleTab && idx == bubbleIdx) {
      _loadBubbles();
    }
  }

  /// tab 数量跟随「自由涂鸦」开关(开 = 3 个;默认关 = 2 个:设置+活页本)。
  /// 长度变化必须重建 controller,否则 TabBar/TabBarView 断言崩溃。
  void _syncTabCount() {
    final want = 2 + (_showFreeCanvas ? 1 : 0) + (_showBubbleTab ? 1 : 0);
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
                  // ── 书写: 最常用的放最前 ──
                  _buildSection(AppLocalizations.of(context)!.pen, _buildPenSection()),
                  const SizedBox(height: 16),
                  _buildSection(AppLocalizations.of(context)!.canvasLook, _buildAppearanceSection()),
                  const SizedBox(height: 16),
                  // ── 活页本: 外观 / 翻页动效 / 缩略图 / 真穿透 ──
                  _buildSection(AppLocalizations.of(context)!.notebookLook, Column(children: [
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
                        onSelectionChanged: (s2) {
                          setState(() => _notebookGlass = s2.first);
                          _setSetting('notebookStyle', s2.first);
                        },
                      ),
                    ]),
                    if (Platform.isMacOS) ...[
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
                          onSelectionChanged: (sel) {
                            setState(() => _flipEffect = sel.first);
                            _setSetting('flipEffect', sel.first);
                          },
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
                      onChanged: (v) {
                        setState(() => _minimapEnabled = v);
                        _setSetting('minimap', v);
                      },
                    )),
                    const Divider(height: 20),
                    _tile(SwitchListTile(
                      title: Text(AppLocalizations.of(context)!.truePassThroughExperimental, style: TextStyle(fontSize: 14)),
                      subtitle: Text(AppLocalizations.of(context)!.cardsRevealTheDesktop, style: TextStyle(fontSize: 12)),
                      value: _panelTransparent && _notebookGlass,
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      onChanged: _notebookGlass
                          ? (v) {
                              setState(() => _panelTransparent = v);
                              _setSetting('panelTransparent', v);
                            }
                          : null,
                    )),
                  ])),
                  const SizedBox(height: 16),
                  // ── 录制与导出 ──
                  _buildSection(AppLocalizations.of(context)!.recordExport, _buildRecordExportSection()),
                  const SizedBox(height: 16),
                  _buildSection(AppLocalizations.of(context)!.dataBackup, _buildBackupButtons()),
                  const SizedBox(height: 16),
                  if (_chatIntegration)
                    _buildSection(AppLocalizations.of(context)!.sharedCanvas, _tile(SwitchListTile(
                      title: Text(AppLocalizations.of(context)!.streamDoodlesLive, style: TextStyle(fontSize: 14)),
                      subtitle: Text(AppLocalizations.of(context)!.pushedToThePeer,
                          style: TextStyle(fontSize: 12)),
                      value: _shareCanvas,
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      onChanged: (v) {
                        setState(() => _shareCanvas = v);
                        _setSetting('shareCanvas', v);
                      },
                    ))),
                  if (_chatIntegration) const SizedBox(height: 16),
                  _buildSection(AppLocalizations.of(context)!.doodleIdentity, _buildChatAuthSection()),
                  const SizedBox(height: 16),
                  // ── 系统与辅助 ──
                  _buildSection(AppLocalizations.of(context)!.systemAndAssist, _buildSystemSection()),
                  const SizedBox(height: 16),
                  _buildSection(AppLocalizations.of(context)!.about, _buildAboutSection()),
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
                    Platform.isWindows
                        ? AppLocalizations.of(context)!.hotkeysCtrlAlt
                        : AppLocalizations.of(context)!.hotkeys,
                    _buildHotkeyGrid(),
                  ),
                ],
              ),
            )),
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
  bool _pdfExporting = false;
}
