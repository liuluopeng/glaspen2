part of 'main.dart';

// ── 桥接层:macOS FRB 直调 / Windows 命名管道 ──

abstract class SettingsBridge {
  Future<Map<dynamic, dynamic>> getSettings();
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
  /// 页面详情: 整页 PNG 字节(白底 1x, 不落盘); highlight = 选中笔迹描蓝
  Future<Uint8List?> exportPagePngBytes(int screenId, {List<int> highlight = const []});
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
      'rainbow': s.rainbow,
      'launchAtLogin': s.launchAtLogin,
      'frostedGlass': s.frostedGlass,
      'grid': s.grid,
      'gridFollowStrokes': s.gridFollowStrokes,
      'glassFollowStrokes': s.glassFollowStrokes,
      'softShadow': s.softShadow,
      'invertInk': s.invertInk,
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
      'shareCanvas': s.shareCanvas,
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
  Future<void> setSetting(String key, dynamic value) async {
    await _init();
    await rust.setSetting(key: key, valueJson: jsonEncode(value));
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
  Future<Uint8List?> exportPagePngBytes(int screenId, {List<int> highlight = const []}) async {
    await _init();
    final bytes = await rust.pagePngBytes(
        screenId: screenId, highlight: frb.Int64List.fromList(highlight));
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
      } else if (type == 'downloadUpdate_frame') {
        // 自动更新进度帧 → 下载流;done 帧后关流
        final id = (msg['reqId'] as num?)?.toInt() ?? 0;
        final ctl = _downloadCtl;
        if (ctl != null && id == _downloadReqId && !ctl.isClosed) {
          final d = msg['data'];
          ctl.add(d is Map<dynamic, dynamic> ? d : const {});
          if (d is Map<dynamic, dynamic> && d['done'] == true) {
            ctl.close();
            _downloadCtl = null;
            _downloadReqId = null;
          }
        }
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
    _writeData('${jsonEncode(msg)}\n');
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
  Future<bool> reorderPage(int screenId, int anchorId, {required bool before}) async {
    final r = await _request('reorderPage', {
      'screenId': screenId, 'anchorId': anchorId, 'before': before,
    });
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<Uint8List?> exportPagePngBytes(int screenId, {List<int> highlight = const []}) async {
    debugPrint('[Pipe] exportPagePngBytes: Windows 管道未接入, 忽略');
    return null;
  }

  @override
  Future<List<int>> lassoSelect(int screenId, List<(double, double)> poly) async {
    final r = await _request('lassoSelect', {
      'screenId': screenId,
      'value': poly.map((p) => [p.$1, p.$2]).toList(),
    });
    return (r['ids'] as List?)?.map((e) => (e as num).toInt()).toList() ?? const [];
  }

  @override
  Future<bool> moveStrokes(int screenId, List<int> ids, double dx, double dy) async {
    final r = await _request('moveStrokes', {
      'screenId': screenId, 'value': {'ids': ids, 'dx': dx, 'dy': dy},
    });
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> moveStrokesToPage(
      int screenId, List<int> ids, int targetScreenId, double dx, double dy) async {
    final r = await _request('moveStrokesToPage', {
      'screenId': screenId,
      'value': {'ids': ids, 'targetScreenId': targetScreenId, 'dx': dx, 'dy': dy},
    });
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> deleteStrokes(int screenId, List<int> ids) async {
    final r = await _request('deleteStrokes', {
      'screenId': screenId, 'value': {'ids': ids},
    });
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<String> copyStrokes(int screenId, List<int> ids) async {
    final r = await _request('copyStrokes', {
      'screenId': screenId, 'value': {'ids': ids},
    });
    return r['payload'] as String? ?? '';
  }

  @override
  Future<int> pasteStrokes(int screenId, String payload, double cx, double cy) async {
    // 管道版: 平移在 Dart 侧做完再发(payload 已含 cx/cy 时由调用方保证)
    final r = await _request('pasteStrokes', {
      'screenId': screenId, 'value': {'payload': payload},
    });
    return (r['count'] as num?)?.toInt() ?? 0;
  }

  @override
  Future<List<PageInfo>> listPages() async {
    final r = await _request('listPages', null);
    final data = r['data'];
    if (data is! List) return const [];
    return data
        .map((e) => PageInfo.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<Map<int, Uint8List>> getPageThumbnails(List<int> ids, int maxSize) async {
    if (ids.isEmpty) return const {};
    final r = await _request('getPageThumbnails', {
      'ids': ids, 'maxSize': maxSize,
    });
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
    final r = await _request('deletePage', {'screenId': screenId});
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<bool> exportPagePng(int screenId) async {
    debugPrint('[Pipe] exportPagePng: Windows 管道未接入, 忽略');
    return false;
  }

  @override
  Future<bool> exportPageSvg(int screenId) async {
    debugPrint('[Pipe] exportPageSvg: Windows 管道未接入, 忽略');
    return false;
  }

  @override
  Future<bool> exportSelectedPdf(List<int> ids) async {
    debugPrint('[Pipe] exportSelectedPdf: Windows 管道未接入, 忽略');
    return false;
  }

  @override
  Future<List<int>> ocrSearch(String query) async {
    debugPrint('[Pipe] ocrSearch: Windows 管道未接入, 忽略');
    return const [];
  }

  @override
  Future<bool> exportPdf() async {
    final r = await _request('exportPdf', null);
    return r['ok'] == 1 || r['ok'] == true;
  }

  @override
  Future<(bool, String)> backupNow() async {
    // 库在覆盖层进程里,备份经管道在那边执行(与 macOS FRB 同一核心函数)
    final r = await _request('backupNow', null);
    return (r['ok'] == 1 || r['ok'] == true, (r['message'] as String?) ?? '');
  }

  @override
  Future<(bool, String)> restoreLatestBackup() async {
    final r = await _request('restoreLatestBackup', null);
    return (r['ok'] == 1 || r['ok'] == true, (r['message'] as String?) ?? '');
  }

  @override
  Future<String> appVersion() async {
    final r = await _request('appVersion', null);
    return (r['version'] as String?) ?? '';
  }

  @override
  Future<String> testChatLogin() async {
    // 经管道在覆盖层进程里强制登录一次;配置已先经 setSetting 保存
    final r = await _request('testChatLogin', null);
    if (r['ok'] == 1 || r['ok'] == true) return '';
    return (r['message'] as String?) ?? '登录失败';
  }


  @override
  Future<Map<dynamic, dynamic>> checkUpdate() async =>
      _request('checkUpdate', null);

  @override
  Future<void> openUrl(String url) async {
    if (!_connected) return;
    _writeData('${jsonEncode({'type': 'openUrl', 'url': url})}\n');
  }

  // 自动更新:进度帧经 downloadUpdate_frame 推送;取消订阅 = 发
  // cancelDownload(Rust 侧 on_progress 偷看管道输入后停下载并删 .part)。
  StreamController<Map<dynamic, dynamic>>? _downloadCtl;
  int? _downloadReqId;

  @override
  Stream<Map<dynamic, dynamic>> downloadUpdate() {
    final ctl = StreamController<Map<dynamic, dynamic>>(onCancel: () {
      _writeData('${jsonEncode({'type': 'cancelDownload'})}\n');
      _downloadCtl = null;
      _downloadReqId = null;
    });
    _downloadCtl = ctl;
    final id = ++_reqSeq;
    _downloadReqId = id;
    _writeData('${jsonEncode({'type': 'downloadUpdate', 'reqId': id})}\n');
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
    return _request('applyUpdate', null);
  }

  @override
  Future<void> navigateToPage(int screenId) async {
    if (!_connected) return;
    _writeData('${jsonEncode({'type': 'navigateToPage', 'screenId': screenId})}\n');
  }

  @override
  Future<void> triggerHotkey(String key) async {
    if (!_connected) return;
    _writeData('${jsonEncode({'type': 'hotkey', 'key': key})}\n');
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
      _writeData('${jsonEncode({'type': 'getSettings'})}\n');
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
      _writeData('${jsonEncode({'type': 'setSetting', 'key': key, 'value': value})}\n');
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
