part of 'main.dart';

// ── 桥接层:macOS FRB 直调 / Windows 命名管道 ──

/// 面板 UI 状态文件目录(平台分支集中在 bridge 工厂, parity 测试要求)。
///
/// 优先 GLASPEN2_DB_PATH 的父目录(测试隔离), 否则沿用平台应用支持目录。
String? panelStateDirPath() {
  final dbPath = Platform.environment['GLASPEN2_DB_PATH'];
  if (dbPath != null && dbPath.isNotEmpty) {
    return File(dbPath).parent.path;
  }
  if (Platform.isMacOS) {
    final home = Platform.environment['HOME'];
    if (home == null || home.isEmpty) return null;
    return '$home/Library/Application Support/glaspen2';
  }
  if (Platform.isWindows) {
    final appData = Platform.environment['LOCALAPPDATA'] ??
        Platform.environment['APPDATA'];
    if (appData == null || appData.isEmpty) return null;
    return appData + r'\glaspen2';
  }
  return null;
}

abstract class SettingsBridge {
  Future<Map<dynamic, dynamic>> getSettings();
  /// 平台 UI 配置(默认键位/保留组合/修饰键名/字号/功能可用性等)。
  /// schema 见 crates/glaspen-core/src/ui.rs;macOS 走 FRB,Windows 走管道。
  Future<Map<String, dynamic>> uiConfig();
  /// 「气泡消息」tab: 全部已完结消息的 JSON(分段规则见 core export/bubbles.rs)
  Future<String> getBubbles();
  /// 把进行中的段强制结为一条消息(打开气泡 tab 时调用)
  Future<void> flushBubbles();
  Future<void> setSetting(String key, dynamic value);
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback);
  /// 连接建立后回调(Windows 管道异步连接;用于连接后重新拉取设置)
  void Function()? onConnected;
  /// Content tab: 页面列表
  Future<List<PageInfo>> listPages();
  /// 活页本重排: 把某页移到锚点页前/后(本子内前后移)
  Future<bool> reorderPage(int screenId, int anchorId, {required bool before});

  // ── 页面详情: 圈选 / 移动 / 复制粘贴 / 删除所选 ──
  Future<List<int>> lassoSelect(int screenId, List<(double, double)> poly);
  Future<bool> moveStrokes(int screenId, List<int> ids, double dx, double dy);
  Future<bool> moveStrokesToPage(
      int screenId, List<int> ids, int targetScreenId, double dx, double dy);
  Future<bool> deleteStrokes(int screenId, List<int> ids);
  Future<String> copyStrokes(int screenId, List<int> ids);
  Future<int> pasteStrokes(int screenId, String payload, double cx, double cy);
  /// Content tab: 一次取多页缩略图(id → PNG);无内容的页不会出现在结果里
  Future<Map<int, Uint8List>> getPageThumbnails(List<int> ids, int maxSize);
  /// 页面详情: 页+边距 PNG 字节(透明底, 不落盘); highlight = 选中描蓝
  Future<Uint8List?> exportPagePngBytes(int screenId,
      {List<int> highlight = const [], double margin = 0});
  /// 删除一页及其笔迹
  Future<bool> deletePage(int screenId);
  /// 导出单页 PNG(白底 2x)到桌面
  Future<bool> exportPagePng(int screenId);
  /// 导出单页 SVG(内容包围盒)到桌面
  Future<bool> exportPageSvg(int screenId);
  /// 勾选的页合成单个 PDF 到桌面
  Future<bool> exportSelectedPdf(List<int> ids);
  /// OCR 全文搜索 → 匹配页 id
  Future<List<int>> ocrSearch(String query);
  /// 跳转到指定页面并恢复笔迹(继续绘画)
  Future<void> navigateToPage(int screenId);
  /// 触发一个快捷键动作(与设置面板热键按钮等价)
  Future<void> triggerHotkey(String key);
  /// 无限画布总览:返回 {png: Uint8List, rect: [x,y,w,h]};空画布返回 {}
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768});
  /// 镜头回到原点 + 100%,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasHome();
  /// 镜头居中到内容包围盒,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasCenter();
  /// 手动新建无限画布:清空内容 + 镜头回原点,返回新的总览载荷
  Future<Map<dynamic, dynamic>> canvasNew();
  /// 导出全部页面为 PDF,返回是否成功
  Future<bool> exportPdf();
  /// 把全部数据备份到桌面;返回 (是否成功, 给用户看的信息)
  Future<(bool, String)> backupNow();
  /// 整库快照推送到 NAS(axum);返回 (是否成功, 给用户看的信息)
  Future<(bool, String)> nasBackupNow();
  /// 从桌面上最新的备份合并恢复;返回 (是否成功, 给用户看的信息)
  Future<(bool, String)> restoreLatestBackup();
  /// 当前版本号(编译进二进制);取不到时返回空串
  Future<String> appVersion();
  /// 检查更新: {ok, current, latest, hasUpdate, url, error}
  Future<Map<dynamic, dynamic>> checkUpdate();
  /// 用系统默认浏览器打开 http(s) URL(「打开下载页」)
  Future<void> openUrl(String url);
  /// 下载最新安装包:每帧 {received, total, done, error, path};
  /// **取消对流的订阅 = 取消下载**(Rust 侧会删掉 .part)
  Stream<Map<dynamic, dynamic>> downloadUpdate();
  /// 解包缓存里的 DMG;{ok, message=暂存 .app 路径或失败原因}
  Future<Map<dynamic, dynamic>> stageUpdate(String tag);
  /// 拉起更新帮手并退出本程序(成功**不返回** —— 进程就地结束)
  Future<Map<dynamic, dynamic>> applyUpdate();
  /// 涂鸦身份「测试登录」:用已保存的配置强制登录一次。
  /// 返回空串 = 成功,否则为可读失败原因。
  Future<String> testChatLogin();
  void dispose();
}

/// macOS:直接用 flutter_rust_bridge 调用同一进程内的 Rust。
///
/// 设置面板是嵌在 glaspen2 主程序里的 Flutter 视图,Rust 代码就在主可执行
/// 文件内,所以用 `DynamicLibrary.process()` 解析符号即可 —— 不再有
/// Flutter MethodChannel,也没有 JSON 中转和逐次平台线程往返。
class _FrbBridge extends SettingsBridge {
  void Function(Map<dynamic, dynamic>)? _onChanged;
  Future<void>? _ready;
  StreamSubscription<rust.Settings>? _settingsSub;

  /// 初始化 FRB(幂等):符号来自主可执行文件,不是单独的 dylib。
  Future<void> _init() {
    return _ready ??= RustLib.init(
      externalLibrary: frb.ExternalLibrary.process(iKnowHowToUseIt: true),
    );
  }

  /// 设置项在 Rust 侧是强类型结构体,这里转回 UI 使用的 Map 形状。
  static Map<dynamic, dynamic> _settingsToMap(rust.Settings? s) {
    if (s == null) return const {};
    return <dynamic, dynamic>{
      'color': s.color,
      'width': s.width,
      'launchAtLogin': s.launchAtLogin,
      'frostedGlass': s.frostedGlass,
      'grid': s.grid,
      'gridFollowStrokes': s.gridFollowStrokes,
      'glassFollowStrokes': s.glassFollowStrokes,
      'softShadow': s.softShadow,
      'invertInk': s.invertInk,
      'notebookStyle': s.notebookStyle,
      'panelTransparent': s.panelTransparent,
      'invertFps': s.invertFps,
      'pressureMonitor': s.pressureMonitor,
      'outline': s.outline,
      'infiniteCanvas': s.infiniteCanvas,
      'minimap': s.minimap,
      'gridSize': s.gridSize,
      'gridDivider': s.gridDivider,
      'flipEffect': s.flipEffect,
      'gifFps': s.gifFps,
      'gifResolution': s.gifResolution,
      'gifSpeed': s.gifSpeed,
      'gifEndMode': s.gifEndMode,
      'chatApiBase': s.chatApiBase,
      'chatUser': s.chatUser,
      'chatHasPassword': s.chatHasPassword,
      'chatIntegration': s.chatIntegration,
      'showFreeCanvas': s.showFreeCanvas,
      'showBubbleTab': s.showBubbleTab,
      'bubbleReplaySpeed': s.bubbleReplaySpeed,
      'bubbleReplayHoldLast': s.bubbleReplayHoldLast,
      'bubbleReplayAuto': s.bubbleReplayAuto,
      'bubbleAutoNewPage': s.bubbleAutoNewPage,
      // nas_url 曾漏映射 → macOS 面板 NAS 地址永远回显空(由 core settings 契约测试发现)。
      'nasUrl': s.nasUrl,
      'shareCanvas': s.shareCanvas,
      'language': s.language,
      'shortcuts': s.shortcuts,
    };
  }

  /// 总览载荷 → UI 期望的 Map;空画布(rust 侧 None)返回 {}。
  static Map<dynamic, dynamic> _payloadToMap(rust.CanvasPayload? p) {
    if (p == null) return const {};
    return <dynamic, dynamic>{
      'png': p.png,
      'rect': p.rect.toList(),
    };
  }

  @override
  Future<Map<dynamic, dynamic>> getSettings() async {
    await _init();
    return _settingsToMap(await rust.getSettings());
  }

  @override
  Future<Map<String, dynamic>> uiConfig() async {
    await _init();
    return decodeUiConfigFrame(await rust.uiConfigPb());
  }

  @override
  Future<String> getBubbles() async {
    await _init();
    final msg = decodePanelMessage(await rust.bubbleMessages());
    return jsonEncode(msg.data);
  }

  @override
  Future<void> flushBubbles() async {
    await _init();
    return rust.bubbleFlush();
  }

  @override
  Future<void> setSetting(String key, dynamic value) async {
    await _init();
    await rust.setSetting(key: key, value: encodeSettingValue(value));
  }

  @override
  void onSettingsChanged(void Function(Map<dynamic, dynamic> s) callback) {
    _onChanged = callback;
    _init().then((_) {
      _settingsSub = rust.settingsChanged().listen(
            (s) => _onChanged?.call(_settingsToMap(s)),
            onError: (Object e) => debugPrint('[FRB] settings stream error: $e'),
          );
    }).catchError((Object e) {
      debugPrint('[FRB] init failed, settings updates disabled: $e');
    });
  }

  @override
  Future<bool> reorderPage(int screenId, int anchorId, {required bool before}) async {
    await _init();
    return rust.reorderPage(screenId: screenId, anchorId: anchorId, before: before);
  }

  @override
  Future<Uint8List?> exportPagePngBytes(int screenId,
      {List<int> highlight = const [], double margin = 0}) async {
    await _init();
    final bytes = await rust.pagePngBytes(
        screenId: screenId,
        highlight: frb.Int64List.fromList(highlight),
        margin: margin);
    return bytes.isEmpty ? null : Uint8List.fromList(bytes);
  }

  @override
  Future<List<int>> lassoSelect(int screenId, List<(double, double)> poly) async {
    await _init();
    final ids = await rust.lassoSelect(
        screenId: screenId,
        poly: poly.map((p) => (p.$1, p.$2)).toList());
    return ids.toList().map((e) => e.toInt()).toList();
  }

  @override
  Future<bool> moveStrokes(int screenId, List<int> ids, double dx, double dy) async {
    await _init();
    return rust.moveStrokes(
        screenId: screenId,
        ids: frb.Int64List.fromList(ids),
        dx: dx,
        dy: dy);
  }

  @override
  Future<bool> moveStrokesToPage(
      int screenId, List<int> ids, int targetScreenId, double dx, double dy) async {
    await _init();
    return rust.moveStrokesToPage(
        screenId: screenId,
        ids: frb.Int64List.fromList(ids),
        targetScreenId: targetScreenId,
        dx: dx,
        dy: dy);
  }

  @override
  Future<bool> deleteStrokes(int screenId, List<int> ids) async {
    await _init();
    return rust.deleteStrokes(
        screenId: screenId, ids: frb.Int64List.fromList(ids));
  }

  @override
  Future<String> copyStrokes(int screenId, List<int> ids) async {
    await _init();
    return rust.copyStrokesPayload(
        screenId: screenId, ids: frb.Int64List.fromList(ids));
  }

  @override
  Future<int> pasteStrokes(int screenId, String payload, double cx, double cy) async {
    await _init();
    return rust.pasteStrokes(
        screenId: screenId, payload: payload, cx: cx, cy: cy);
  }

  @override
  Future<List<PageInfo>> listPages() async {
    await _init();
    final pages = await rust.listPages();
    return pages
        .map((p) => PageInfo(id: p.id.toInt(), w: p.width, h: p.height, strokeCount: p.strokeCount.toInt()))
        .toList();
  }

  @override
  Future<Map<int, Uint8List>> getPageThumbnails(List<int> ids, int maxSize) async {
    if (ids.isEmpty) return const {};
    await _init();
    final thumbs = await rust.pageThumbnails(
      ids: frb.Int64List.fromList(ids),
      maxSize: maxSize,
    );
    return {for (final t in thumbs) t.id: t.png};
  }

  @override
  Future<bool> deletePage(int screenId) async {
    await _init();
    return rust.deletePage(screenId: screenId);
  }

  @override
  Future<bool> exportPagePng(int screenId) async {
    await _init();
    return rust.exportPagePng(screenId: screenId);
  }

  @override
  Future<bool> exportPageSvg(int screenId) async {
    await _init();
    return rust.exportPageSvg(screenId: screenId);
  }

  @override
  Future<bool> exportSelectedPdf(List<int> ids) async {
    await _init();
    return rust.exportSelectedPdf(ids: frb.Int64List.fromList(ids));
  }

  @override
  Future<List<int>> ocrSearch(String query) async {
    await _init();
    return (await rust.ocrSearch(query: query)).map((e) => e.toInt()).toList();
  }

  @override
  Future<void> navigateToPage(int screenId) async {
    await _init();
    await rust.navigateToPage(screenId: screenId);
  }

  @override
  Future<void> triggerHotkey(String key) async {
    await _init();
    await rust.triggerHotkey(key: key);
  }

  @override
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768}) async {
    await _init();
    return _payloadToMap(await rust.canvasOverview(
      w: w, h: h, action: rust.CanvasAction.current));
  }

  @override
  Future<Map<dynamic, dynamic>> canvasHome() async {
    await _init();
    return _payloadToMap(await rust.canvasOverview(
      w: 1024, h: 768, action: rust.CanvasAction.home));
  }

  @override
  Future<Map<dynamic, dynamic>> canvasCenter() async {
    await _init();
    return _payloadToMap(await rust.canvasOverview(
      w: 1024, h: 768, action: rust.CanvasAction.center));
  }

  @override
  Future<Map<dynamic, dynamic>> canvasNew() async {
    await _init();
    return _payloadToMap(await rust.canvasOverview(
      w: 1024, h: 768, action: rust.CanvasAction.new_));
  }

  @override
  Future<bool> exportPdf() async {
    await _init();
    return rust.exportPdf();
  }

  @override
  Future<(bool, String)> backupNow() async {
    await _init();
    final r = await rust.backupNow();
    return (r.ok, r.message);
  }

  @override
  Future<(bool, String)> nasBackupNow() async {
    await _init();
    final v = decodeNasBackupFrame(await rust.nasBackupNow());
    final ok = v['ok'] == true;
    final text = ok
        ? '${(v['size_bytes'] as num? ?? 0) ~/ 1024} KB · ${(v['stroke_count'] as num? ?? 0)} 笔'
        : '${v['error'] ?? ''}${(v['hint'] as String?)?.isNotEmpty == true ? '(${v['hint']})' : ''}';
    return (ok, text);
  }

  @override
  Future<(bool, String)> restoreLatestBackup() async {
    await _init();
    final r = await rust.restoreLatestBackup();
    return (r.ok, r.message);
  }

  @override
  Future<String> appVersion() async {
    await _init();
    return rust.appVersion();
  }

  @override
  Future<String> testChatLogin() async {
    await _init();
    return rust.testChatLogin();
  }

  @override
  Future<Map<dynamic, dynamic>> checkUpdate() async {
    await _init();
    final r = await rust.checkUpdate();
    return {
      'ok': r.ok,
      'current': r.current,
      'latest': r.latest,
      'hasUpdate': r.hasUpdate,
      'url': r.url,
      'error': r.error,
      'notes': r.notes,
      'assets': [
        for (final a in r.assets)
          {'name': a.name, 'url': a.url, 'size': a.size.toInt(), 'sha256': a.sha256},
      ],
    };
  }

  @override
  Future<void> openUrl(String url) async {
    await _init();
    await rust.openUrl(url: url);
  }

  @override
  Stream<Map<dynamic, dynamic>> downloadUpdate() async* {
    await _init();
    // async*: 监听者取消订阅时内层流一并取消 → Rust 侧 sink 推不进去
    // → 回调返回 false → 下载中止并删除 .part。
    yield* rust.downloadUpdate().map((p) => <dynamic, dynamic>{
          'received': p.received.toInt(),
          'total': p.total.toInt(),
          'done': p.done,
          'error': p.error,
          'path': p.path,
        });
  }

  @override
  Future<Map<dynamic, dynamic>> stageUpdate(String tag) async {
    await _init();
    final r = await rust.stageUpdate(tag: tag);
    return {'ok': r.ok, 'message': r.message};
  }

  @override
  Future<Map<dynamic, dynamic>> applyUpdate() async {
    await _init();
    final r = await rust.applyUpdate();
    return {'ok': r.ok, 'message': r.message};
  }

  @override
  void dispose() {
    _settingsSub?.cancel();
    _settingsSub = null;
  }
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
class _NamedPipeBridge extends SettingsBridge {
  int _handle = -1; // Windows HANDLE
  final _decoder = PanelStreamDecoder();
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
      // 同步句柄: 读取用 PeekNamedPipe + ReadFile(最后一个参数传 NULL)。
      // 若带 FILE_FLAG_OVERLAPPED 打开而 ReadFile/WriteFile 传 NULL
      // OVERLAPPED, Win32 会返回 ERROR_INVALID_PARAMETER, 整条设置通道
      // 都收不到数据。
      const flags = 0;

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
      _writeBytes(Uint8List.fromList(panelMagic));
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

    final totalAvail = calloc<Uint32>();
    final ok = _peekNamedPipe(_handle, nullptr, 0, nullptr, totalAvail, nullptr);
    final avail = totalAvail.value;
    calloc.free(totalAvail);

    if (ok == 0) {
      _connected = false;
      _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
      return;
    }
    if (avail == 0) return;

    final toRead = avail > 65536 ? 65536 : avail;
    final buf = calloc<Uint8>(toRead);
    final bytesRead = calloc<Uint32>();
    final success = _readFile(_handle, buf, toRead, bytesRead, nullptr);
    final count = bytesRead.value;
    calloc.free(bytesRead);

    if (success != 0 && count > 0) {
      final chunk = Uint8List(count);
      for (int i = 0; i < count; i++) {
        chunk[i] = buf[i];
      }
      try {
        for (final msg in _decoder.feed(chunk)) {
          _handlePanelMessage(msg);
        }
      } catch (e) {
        debugPrint('[Settings] protobuf decode error: $e');
        _connected = false;
        _reconnectTimer = Timer(const Duration(seconds: 2), _connect);
      }
    }
    calloc.free(buf);
  }

  void _handlePanelMessage(PanelMessage msg) {
    final kind = msg.kind;
    final data = msg.data;
    if (kind == 'onSettingsChanged' && _onChanged != null && data is Map) {
      _onChanged!(Map<dynamic, dynamic>.from(data));
    } else if (kind == 'downloadUpdate_frame') {
      final id = msg.requestId;
      final ctl = _downloadCtl;
      if (ctl != null && id == _downloadReqId && !ctl.isClosed) {
        ctl.add(data is Map<dynamic, dynamic> ? data : const {});
        if (data is Map<dynamic, dynamic> && data['done'] == true) {
          ctl.close();
          _downloadCtl = null;
          _downloadReqId = null;
        }
      }
    } else if (kind == 'getSettings_response' && _settingsCompleter != null) {
      _settingsCompleter!.complete(
        data is Map ? Map<dynamic, dynamic>.from(data) : <dynamic, dynamic>{},
      );
      _settingsCompleter = null;
    } else if (kind.endsWith('_response')) {
      final c = _pendingReqs.remove(msg.requestId);
      if (c != null) {
        if (data is Map) {
          c.complete(Map<dynamic, dynamic>.from(data));
        } else if (data is List) {
          c.complete({'data': data});
        } else {
          c.complete({'data': data});
        }
      }
    }
  }

  Future<Map<dynamic, dynamic>> _requestTyped(
      List<int> Function(int id) encode) async {
    if (!_connected) return {};
    final id = ++_reqSeq;
    final c = Completer<Map<dynamic, dynamic>>();
    _pendingReqs[id] = c;
    _writeBytes(encode(id));
    try {
      return await c.future.timeout(
        const Duration(seconds: 15),
        onTimeout: () {
          _pendingReqs.remove(id);
          return <dynamic, dynamic>{};
        },
      );
    } catch (_) {
      _pendingReqs.remove(id);
      return {};
    }
  }


  @override
  Future<bool> reorderPage(int screenId, int anchorId, {required bool before}) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'reorderPage',
          screenId: screenId,
          anchorId: anchorId,
          before: before,
        ));
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<Uint8List?> exportPagePngBytes(int screenId,
      {List<int> highlight = const [], double margin = 0}) async {
    // 面板已按 features.pageExport 隐藏入口;走到这里说明 UI 门控漏了 —— 抛错
    // 让问题在开发/测试期立刻暴露,而不是返回 null 静默空壳。
    throw UnsupportedError(
        'Windows 侧尚未实现 exportPagePngBytes (uiConfig.features.pageExport=false; UI 应已隐藏该入口)');
  }

  @override
  Future<List<int>> lassoSelect(int screenId, List<(double, double)> poly) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'lassoSelect',
          screenId: screenId,
          lasso: [for (final p in poly) ...[p.$1, p.$2]],
        ));
    return (r['ids'] as List?)?.map((e) => (e as num).toInt()).toList() ?? const [];
  }

  @override
  Future<bool> moveStrokes(int screenId, List<int> ids, double dx, double dy) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'moveStrokes',
          screenId: screenId,
          ids: ids,
          dx: dx,
          dy: dy,
        ));
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> moveStrokesToPage(
      int screenId, List<int> ids, int targetScreenId, double dx, double dy) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'moveStrokesToPage',
          screenId: screenId,
          ids: ids,
          targetScreenId: targetScreenId,
          dx: dx,
          dy: dy,
        ));
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> deleteStrokes(int screenId, List<int> ids) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'deleteStrokes',
          screenId: screenId,
          ids: ids,
        ));
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<String> copyStrokes(int screenId, List<int> ids) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'copyStrokes',
          screenId: screenId,
          ids: ids,
        ));
    return r['payload'] as String? ?? '';
  }

  @override
  Future<int> pasteStrokes(int screenId, String payload, double cx, double cy) async {
    // 管道版: 平移在 Dart 侧做完再发(payload 已含 cx/cy 时由调用方保证)
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'pasteStrokes',
          screenId: screenId,
          payload: payload,
          cx: cx,
          cy: cy,
        ));
    return (r['count'] as num?)?.toInt() ?? 0;
  }

  @override
  Future<List<PageInfo>> listPages() async {
    final r = await _requestTyped(encodeListPagesRequest);
    final data = r['data'];
    if (data is! List) return const [];
    return data
        .map((e) => PageInfo.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<Map<int, Uint8List>> getPageThumbnails(List<int> ids, int maxSize) async {
    if (ids.isEmpty) return const {};
    final r = await _requestTyped((id) => encodePageThumbnailsRequest(id, ids, maxSize));
    final entries = r['entries'];
    if (entries is List) {
      final out = <int, Uint8List>{};
      for (final raw in entries) {
        if (raw is! Map) continue;
        final id = (raw['id'] as num?)?.toInt();
        final png = raw['png'];
        if (id == null) continue;
        if (png is Uint8List) {
          out[id] = png;
        } else if (png is List) {
          out[id] = Uint8List.fromList(png.cast<int>());
        }
      }
      return out;
    }
    // 兼容旧字段(仅回滚期; 新后端不再发)。
    final blob = r['blob'] as String?;
    if (blob == null || blob.isEmpty) return const {};
    try {
      return parseThumbnailBlob(base64Decode(blob));
    } catch (e) {
      return const {};
    }
  }

  @override
  Future<bool> deletePage(int screenId) async {
    final r = await _requestTyped((id) => encodePageOpRequest(
          id,
          kind: 'deletePage',
          screenId: screenId,
        ));
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> exportPagePng(int screenId) async {
    throw UnsupportedError(
        'Windows 侧尚未实现 exportPagePng (uiConfig.features.pageExport=false; UI 应已隐藏该入口)');
  }

  @override
  Future<bool> exportPageSvg(int screenId) async {
    throw UnsupportedError(
        'Windows 侧尚未实现 exportPageSvg (uiConfig.features.pageExport=false; UI 应已隐藏该入口)');
  }

  @override
  Future<bool> exportSelectedPdf(List<int> ids) async {
    throw UnsupportedError(
        'Windows 侧尚未实现 exportSelectedPdf (uiConfig.features.pageExport=false; UI 应已隐藏该入口)');
  }

  @override
  Future<List<int>> ocrSearch(String query) async {
    throw UnsupportedError(
        'Windows 侧尚未实现 ocrSearch (uiConfig.features.ocrSearch=false; UI 应已隐藏该入口)');
  }

  @override
  Future<bool> exportPdf() async {
    final r = await _requestTyped(encodeExportPdfRequest);
    return r['ok'] == true;
  }

  @override
  Future<(bool, String)> backupNow() async {
    final r = await _requestTyped((id) =>
        encodeBackupCommandRequest(id, pb.BackupCommand.BACKUP_NOW));
    return (r['ok'] == true, (r['message'] as String?) ?? '');
  }

  @override
  Future<(bool, String)> nasBackupNow() async {
    // 库在覆盖层进程里,NAS 备份经管道在那边执行(同一核心函数)
    final r = await _requestTyped(encodeNasBackupRequest);
    final v = r;
    final ok = v['ok'] == true;
    final text = ok
        ? '${((v['size_bytes'] as num? ?? 0) ~/ 1024)} KB · ${(v['stroke_count'] as num? ?? 0)} 笔'
        : '${v['error'] ?? ''}${(v['hint'] as String?)?.isNotEmpty == true ? '(${v['hint']})' : ''}';
    return (ok, text);
  }

  @override
  Future<(bool, String)> restoreLatestBackup() async {
    final r = await _requestTyped((id) =>
        encodeBackupCommandRequest(id, pb.BackupCommand.RESTORE_LATEST));
    return (r['ok'] == true, (r['message'] as String?) ?? '');
  }

  @override
  Future<String> appVersion() async {
    final r = await _requestTyped(encodeAppVersionRequest);
    return (r['version'] as String?) ?? '';
  }

  @override
  Future<String> testChatLogin() async {
    final r = await _requestTyped(encodeChatLoginRequest);
    if (r['ok'] == true) return '';
    return (r['message'] as String?) ?? '登录失败';
  }


  @override
  Future<Map<dynamic, dynamic>> checkUpdate() async =>
      _requestTyped(encodeCheckUpdateRequest);

  @override
  Future<void> openUrl(String url) async {
    if (!_connected) return;
    _writeBytes(encodeOpenUrlCommand(url));
  }

  // 自动更新:进度帧经 downloadUpdate_frame 推送;取消订阅 = 发
  // cancelDownload(Rust 侧 on_progress 偷看管道输入后停下载并删 .part)。
  StreamController<Map<dynamic, dynamic>>? _downloadCtl;
  int? _downloadReqId;

  @override
  Stream<Map<dynamic, dynamic>> downloadUpdate() {
    final ctl = StreamController<Map<dynamic, dynamic>>(onCancel: () {
      _writeBytes(encodeCancelDownloadRequest());
      _downloadCtl = null;
      _downloadReqId = null;
    });
    _downloadCtl = ctl;
    final id = ++_reqSeq;
    _downloadReqId = id;
    _writeBytes(encodeDownloadUpdateRequest(id));
    return ctl.stream;
  }

  @override
  Future<Map<dynamic, dynamic>> stageUpdate(String tag) async {
    // Windows 无 DMG 解包:安装包下载(校验)完即就绪
    return const {'ok': true, 'message': '安装包已就绪'};
  }

  @override
  Future<Map<dynamic, dynamic>> applyUpdate() async {
    // 成功路径:覆盖层进程退出、安装器延迟启动 —— 面板随后被一起带走,
    // 这里的 Future 大概率因管道断开返回空 map(与 macOS「成功不返回」一致)
    return _requestTyped(encodeApplyUpdateRequest);
  }

  @override
  Future<void> navigateToPage(int screenId) async {
    if (!_connected) return;
    _writeBytes(encodeNavigateToPageRequest(screenId));
  }

  @override
  Future<void> triggerHotkey(String key) async {
    if (!_connected) return;
    _writeBytes(encodeHotkeyCommand(key));
  }

  @override
  Future<Map<String, dynamic>> uiConfig() async {
    final r = await _requestTyped((id) =>
        encodeUiConfigRequest(id, kPanelProtocolVersion));
    return {for (final e in r.entries) e.key.toString(): e.value};
  }

  @override
  Future<Map<dynamic, dynamic>> canvasOverview({int w = 1024, int h = 768}) async =>
      _requestTyped((id) =>
          encodeCanvasOverviewRequest(id, w, h, pb.CanvasAction.CANVAS_CURRENT));

  @override
  Future<Map<dynamic, dynamic>> canvasHome() async =>
      _requestTyped((id) =>
          encodeCanvasOverviewRequest(id, 1024, 768, pb.CanvasAction.CANVAS_HOME));

  @override
  Future<Map<dynamic, dynamic>> canvasCenter() async =>
      _requestTyped((id) =>
          encodeCanvasOverviewRequest(id, 1024, 768, pb.CanvasAction.CANVAS_CENTER));

  @override
  Future<Map<dynamic, dynamic>> canvasNew() async =>
      _requestTyped((id) =>
          encodeCanvasOverviewRequest(id, 1024, 768, pb.CanvasAction.CANVAS_NEW));

  bool _writeBytes(List<int> bytes) {
    if (!_connected || _handle == -1) return false;
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
      _writeBytes(encodeGetSettingsRequest(0));
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
  Future<String> getBubbles() async {
    if (!_connected) return '{"messages":[]}';
    final r = await _requestTyped(encodeGetBubblesRequest);
    return jsonEncode(r);
  }

  @override
  Future<void> flushBubbles() async {
    if (!_connected) return;
    await _requestTyped(encodeFlushBubblesRequest);
  }

  @override
  Future<void> setSetting(String key, dynamic value) async {
    if (!_connected) return;
    try {
      _writeBytes(encodeSetSettingRequest(0, key, value));
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

/// 面板 ↔ 后端协议版本; 必须与 core/protocol.rs 的 PROTOCOL_VERSION 同步
/// (cargo test 会扫描校验)。Windows 面板是独立 exe, 新旧混跑时后端会明确拒绝。
const int kPanelProtocolVersion = 1;

/// Create the appropriate bridge for the current platform.
SettingsBridge createBridge() {
  if (Platform.isWindows) {
    // Windows 面板是独立进程,Rust 在覆盖层进程里 → 只能走命名管道
    return _NamedPipeBridge();
  }
  // macOS 面板与 Rust 在同一进程内 → flutter_rust_bridge(无 MethodChannel)
  return _FrbBridge();
}

// ── Data models ──

/// 「立即更新」的状态机:idle → downloading → staging → ready → applying;
/// 任何一步失败都进 failed(错误文案在 _updError)。
enum _UpdPhase { idle, downloading, staging, ready, applying, failed }

/// 缩略图未就位时的占位:一层极淡的纸面色,而不是"图片缺失"图标——
/// 图片到达时只是笔迹浮现,不会有图标跳变。
