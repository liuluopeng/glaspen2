part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 无限画布 tab(自由涂鸦/总览/画布动作) ──

/// 页面编辑 FFI 的哨兵 screen_id: Rust 侧把负数路由到 infinite_* 表。
const int _infiniteCanvasScreenId = -1;

extension _CanvasTabUi on _SettingsPageState {
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

  /// 总览载荷 → png/视口矩形/内容包围盒。rect 现在 8 个数:
  /// [0..3] 视口, [4..7] 内容包围盒(旧 Windows 管道可能只有 4 个)。
  void _applyCanvasPayload(Map<dynamic, dynamic> payload) {
    final png = _pngBytes(payload['png']);
    final r = payload['rect'] as List<dynamic>?;
    final vals = (r == null || r.length < 4)
        ? null
        : r.map((e) => (e as num).toDouble()).toList();
    setState(() {
      _canvasPng = png;
      _canvasRect = vals?.sublist(0, 4);
      _canvasBounds = (vals != null && vals.length >= 8)
          ? vals.sublist(4, 8)
          : const [];
      _canvasLoading = false;
    });
  }

  /// 拉取无限画布总览(当前激活画布按内容包围盒适配 + 当前视口矩形)
  Future<void> _loadCanvasOverview(
      {Map<dynamic, dynamic>? action}) async {
    if (_canvasLoading) return;
    setState(() => _canvasLoading = true);
    try {
      final payload = action ?? await _bridge.canvasOverview();
      if (!mounted) return;
      _applyCanvasPayload(payload);
    } catch (e) {
      debugPrint('[Canvas] overview error: $e');
      if (mounted) {
        setState(() => _canvasLoading = false);
        _toastError(AppLocalizations.of(context)!.loadFailedP1('$e'));
      }
    }
  }

  Future<void> _canvasAction(String action) async {
    // 自由涂鸦按 macOS 设计**只有一张画布, 不存在清空/新建操作**: 这里只提示,
    // 绝不发 canvasNew(以前会清空整片自由涂鸦, 数据事故)。
    if (action == 'new') {
      await showDialog<void>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(_canvasText('新建画布', 'New canvas')),
          content: Text(_canvasText(
              '自由涂鸦只有一张画布，不存在清空/新建操作。',
              'Free doodle has a single canvas — there is no clear/new operation.')),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx),
              child: Text(_canvasText('知道了', 'Got it')),
            ),
          ],
        ),
      );
      return;
    }
    setState(() => _canvasLoading = true);
    try {
      final payload = await (action == 'home'
          ? _bridge.canvasHome()
          : action == 'new'
              ? _bridge.canvasNew()
              : _bridge.canvasCenter());
      if (!mounted) return;
      _applyCanvasPayload(payload);
    } catch (e) {
      debugPrint('[Canvas] $action error: $e');
      if (mounted) setState(() => _canvasLoading = false);
    }
  }

  String _canvasText(String zh, String en) => _language == 'en' ? en : zh;

  // ── 自由涂鸦编辑: 圈选 / 移动 / 复制粘贴 / 删除 ──
  // 复用活页本详情的 lasso 手势与 _DetailPainter; screenId 传 -1,
  // Rust 侧据此把 lasso/move/copy/paste/delete 路由到 infinite_* 表。

  double _canvasMapScale() {
    final img = _canvasEditImage;
    final b = _canvasBounds;
    if (img == null || b.length < 4 || b[2] <= 0 || b[3] <= 0) return 1.0;
    return math.min(img.width / b[2], img.height / b[3]);
  }

  double _canvasMapOx() {
    final img = _canvasEditImage;
    final b = _canvasBounds;
    if (img == null || b.length < 4) return 0.0;
    return (img.width - b[2] * _canvasMapScale()) * 0.5;
  }

  double _canvasMapOy() {
    final img = _canvasEditImage;
    final b = _canvasBounds;
    if (img == null || b.length < 4) return 0.0;
    return (img.height - b[3] * _canvasMapScale()) * 0.5;
  }

  /// 详情画布局部坐标 → 无限画布坐标。
  Offset _canvasViewToCanvas(Offset v) {
    final b = _canvasBounds;
    if (_canvasEditImage == null || b.length < 4 || _canvasEditScale <= 0) {
      return Offset.zero;
    }
    final s = _canvasMapScale();
    final ix = (v.dx - _canvasEditOffset.dx) / _canvasEditScale;
    final iy = (v.dy - _canvasEditOffset.dy) / _canvasEditScale;
    return Offset(
      b[0] + (ix - _canvasMapOx()) / s,
      b[1] + (iy - _canvasMapOy()) / s,
    );
  }

  /// 详情画布上的位移 → 画布坐标位移。
  double _canvasViewDeltaToCanvas(double d) {
    if (_canvasEditScale <= 0) return 0;
    return d / (_canvasEditScale * _canvasMapScale());
  }

  Future<void> _openCanvasEditor() async {
    if (_canvasPng == null || _canvasBounds.length < 4) {
      _toast(_canvasText('画布为空, 没有可编辑的内容', 'Canvas is empty; nothing to edit'));
      return;
    }
    setState(() => _canvasLoading = true);
    Map<dynamic, dynamic> payload = const {};
    try {
      // 编辑图比 tab 总览大一档, 圈选/拖拽精度更好。
      payload = await _bridge.canvasOverview(w: 2048, h: 1536);
    } catch (e) {
      debugPrint('[Canvas] edit overview error: $e');
    }
    if (!mounted) return;
    if (payload.isNotEmpty) _applyCanvasPayload(payload);
    final bytes = _canvasPng;
    if (bytes == null || _canvasBounds.length < 4) {
      setState(() => _canvasLoading = false);
      _toast(_canvasText('画布为空, 没有可编辑的内容', 'Canvas is empty; nothing to edit'));
      return;
    }
    try {
      final codec = await ui.instantiateImageCodec(bytes);
      final frame = await codec.getNextFrame();
      if (!mounted) return;
      setState(() {
        _canvasEditImage = frame.image;
        _canvasEditing = true;
        _canvasLassoPts.clear();
        _canvasSelectedStrokes = [];
        _canvasDragStart = null;
        _canvasUndoMoves.clear();
        _canvasLoading = false;
      });
    } catch (e) {
      debugPrint('[Canvas] edit decode error: $e');
      if (mounted) {
        setState(() => _canvasLoading = false);
        _toastError(AppLocalizations.of(context)!.loadFailedP1('$e'));
      }
    }
  }

  void _closeCanvasEditor() {
    setState(() {
      _canvasEditing = false;
      _canvasEditImage = null;
      _canvasLassoPts.clear();
      _canvasSelectedStrokes = [];
      _canvasDragStart = null;
    });
    unawaited(_loadCanvasOverview());
  }

  Future<void> _reloadCanvasEditor() async {
    if (!_canvasEditing) return;
    Map<dynamic, dynamic> payload = const {};
    try {
      payload = await _bridge.canvasOverview(w: 2048, h: 1536);
    } catch (e) {
      debugPrint('[Canvas] edit reload error: $e');
      return;
    }
    if (!mounted || !_canvasEditing) return;
    if (payload.isEmpty) {
      _closeCanvasEditor();
      _toast(_canvasText('画布已空', 'Canvas is now empty'));
      return;
    }
    _applyCanvasPayload(payload);
    final bytes = _canvasPng;
    if (bytes == null || _canvasBounds.length < 4) {
      _closeCanvasEditor();
      return;
    }
    try {
      final codec = await ui.instantiateImageCodec(bytes);
      final frame = await codec.getNextFrame();
      if (!mounted || !_canvasEditing) return;
      setState(() => _canvasEditImage = frame.image);
    } catch (e) {
      debugPrint('[Canvas] edit reload decode error: $e');
    }
  }

  Future<void> _finishCanvasLasso() async {
    if (!_canvasEditing || _canvasLassoPts.length < 3) {
      setState(() => _canvasLassoPts.clear());
      return;
    }
    final poly = _canvasLassoPts
        .map(_canvasViewToCanvas)
        .map((p) => (p.dx, p.dy))
        .toList();
    final ids = await _bridge.lassoSelect(_infiniteCanvasScreenId, poly);
    if (!mounted || !_canvasEditing) return;
    setState(() => _canvasSelectedStrokes = ids);
    if (ids.isEmpty) {
      setState(() => _canvasLassoPts.clear());
      _toast(_canvasText('圈内没有笔迹', 'No strokes inside'));
    } else {
      // 保留蓝色圈选路径作为选中反馈, 下一次拖拽/圈选时清掉。
      _toast(_canvasText('已选中 ${ids.length} 笔', '${ids.length} stroke(s) selected'));
    }
  }

  Future<void> _finishCanvasDrag() async {
    final start = _canvasDragStart;
    if (start == null || !_canvasEditing || _canvasSelectedStrokes.isEmpty) {
      setState(() => _canvasDragStart = null);
      return;
    }
    final deltaView = _canvasDragNow - start;
    if (deltaView.distance < 1.0) {
      // 有选中时的原地点击 = 取消选择
      setState(() {
        _canvasDragStart = null;
        _canvasSelectedStrokes = [];
        _canvasLassoPts.clear();
      });
      return;
    }
    final dx = _canvasViewDeltaToCanvas(deltaView.dx);
    final dy = _canvasViewDeltaToCanvas(deltaView.dy);
    final ids = List<int>.from(_canvasSelectedStrokes);
    final ok = await _bridge.moveStrokes(_infiniteCanvasScreenId, ids, dx, dy);
    if (!mounted) return;
    if (ok) {
      setState(() {
        _canvasUndoMoves.add((ids, -dx, -dy));
        _canvasDragStart = null;
        _canvasLassoPts.clear();
      });
      await _reloadCanvasEditor();
    } else {
      setState(() => _canvasDragStart = null);
      _toast(_canvasText('移动失败', 'Move failed'));
    }
  }

  Future<void> _copyCanvasSelection() async {
    if (_canvasSelectedStrokes.isEmpty) return;
    final payload = await _bridge.copyStrokes(_infiniteCanvasScreenId, _canvasSelectedStrokes);
    if (!mounted) return;
    setState(() => _clipPayload = payload);
    _toast(payload.isEmpty
        ? _canvasText('没有可复制的内容', 'Nothing to copy')
        : _canvasText('已复制 ${_canvasSelectedStrokes.length} 笔',
            'Copied ${_canvasSelectedStrokes.length} stroke(s)'));
  }

  Future<void> _pasteCanvasAt(Offset viewPos) async {
    if (!_canvasEditing || _clipPayload.isEmpty) {
      _toast(_canvasText('剪贴板是空的', 'Clipboard is empty'));
      return;
    }
    final p = _canvasViewToCanvas(viewPos);
    final n = await _bridge.pasteStrokes(_infiniteCanvasScreenId, _clipPayload, p.dx, p.dy);
    if (!mounted) return;
    if (n > 0) {
      setState(() {
        _canvasSelectedStrokes = [];
        _canvasLassoPts.clear();
        _canvasDragStart = null;
      });
      _toast(_canvasText('已粘贴 $n 笔', 'Pasted $n stroke(s)'));
      await _reloadCanvasEditor();
    } else {
      _toast(_canvasText('粘贴失败', 'Paste failed'));
    }
  }

  Future<void> _deleteCanvasSelection() async {
    if (_canvasSelectedStrokes.isEmpty) return;
    final ids = List<int>.from(_canvasSelectedStrokes);
    final ok = await _bridge.deleteStrokes(_infiniteCanvasScreenId, ids);
    if (!mounted) return;
    if (ok) {
      setState(() {
        _canvasSelectedStrokes = [];
        _canvasLassoPts.clear();
        _canvasDragStart = null;
      });
      _toast(_canvasText('已删除 ${ids.length} 笔', 'Deleted ${ids.length} stroke(s)'));
      await _reloadCanvasEditor();
    } else {
      _toast(_canvasText('删除失败', 'Delete failed'));
    }
  }

  Future<void> _undoCanvasMove() async {
    if (_canvasUndoMoves.isEmpty) {
      _toast(_canvasText('没有可撤销的移动', 'Nothing to undo'));
      return;
    }
    final (ids, dx, dy) = _canvasUndoMoves.removeLast();
    final ok = await _bridge.moveStrokes(_infiniteCanvasScreenId, ids, dx, dy);
    if (!mounted) return;
    if (ok) {
      setState(() {
        _canvasLassoPts.clear();
        _canvasSelectedStrokes = [];
        _canvasDragStart = null;
      });
      await _reloadCanvasEditor();
    } else {
      _toast(_canvasText('撤销失败', 'Undo failed'));
    }
  }

  void _showCanvasEditMenu(Offset globalPos, Offset canvasPos) {
    final sel = _canvasSelectedStrokes.isNotEmpty;
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox;
    final rel = overlay.globalToLocal(globalPos);
    const menuW = 220.0, menuH = 150.0;
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
          PopupMenuItem(
              value: 'copy',
              height: 36,
              child: Row(children: [
                const Icon(Icons.copy, size: 16),
                const SizedBox(width: 8),
                Text(_canvasText('复制所选', 'Copy selection'),
                    style: const TextStyle(fontSize: 13)),
              ])),
        if (_clipPayload.isNotEmpty)
          PopupMenuItem(
              value: 'paste',
              height: 36,
              child: Row(children: [
                const Icon(Icons.paste, size: 16),
                const SizedBox(width: 8),
                Text(_canvasText('粘贴到这里', 'Paste here'),
                    style: const TextStyle(fontSize: 13)),
              ])),
        if (sel)
          PopupMenuItem(
              value: 'delete',
              height: 36,
              child: Row(children: [
                const Icon(Icons.delete_outline, size: 16, color: Colors.red),
                const SizedBox(width: 8),
                Text(_canvasText('删除所选', 'Delete selection'),
                    style: const TextStyle(fontSize: 13)),
              ])),
      ],
    ).then((v) {
      if (v == 'copy') unawaited(_copyCanvasSelection());
      if (v == 'paste') unawaited(_pasteCanvasAt(canvasPos));
      if (v == 'delete') unawaited(_deleteCanvasSelection());
    });
  }

  Widget _buildCanvasEditor() {
    final img = _canvasEditImage;
    if (img == null) return const Center(child: CircularProgressIndicator());
    return Column(children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(8, 2, 8, 2),
        child: Row(children: [
          TextButton.icon(
            onPressed: _closeCanvasEditor,
            icon: const Icon(Icons.arrow_back, size: 16),
            label: Text(_canvasText('总览', 'Overview'),
                style: const TextStyle(fontSize: 13)),
          ),
          const SizedBox(width: 8),
          Text(
            _canvasText('自由涂鸦 · 已选 ${_canvasSelectedStrokes.length} 笔',
                'Free doodle · ${_canvasSelectedStrokes.length} selected'),
            style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
          ),
          const Spacer(),
          TextButton.icon(
            onPressed: _copyCanvasSelection,
            icon: const Icon(Icons.copy, size: 16),
            label: Text(_canvasText('复制', 'Copy'),
                style: const TextStyle(fontSize: 13)),
          ),
          TextButton.icon(
            onPressed: _deleteCanvasSelection,
            icon: const Icon(Icons.delete_outline, size: 16),
            label: Text(_canvasText('删除', 'Delete'),
                style: const TextStyle(fontSize: 13)),
          ),
          TextButton.icon(
            onPressed: _undoCanvasMove,
            icon: const Icon(Icons.undo, size: 16),
            label: Text(_canvasText('撤销移动', 'Undo move'),
                style: const TextStyle(fontSize: 13)),
          ),
        ]),
      ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(
            _canvasText(
              '拖拽圈选；选中后拖动移动；右键复制/粘贴/删除。',
              'Drag to lasso; drag the selection to move; right-click to copy/paste/delete.',
            ),
            style: const TextStyle(fontSize: 11, color: Colors.grey),
          ),
        ),
      ),
      Expanded(
        child: LayoutBuilder(builder: (context, cc) {
          final sw = img.width <= 0 ? 1.0 : cc.maxWidth / img.width;
          final sh = img.height <= 0 ? 1.0 : cc.maxHeight / img.height;
          _canvasEditScale = sw < sh ? sw : sh;
          _canvasEditOffset = Offset(
            (cc.maxWidth - img.width * _canvasEditScale) / 2,
            (cc.maxHeight - img.height * _canvasEditScale) / 2,
          );
          return GestureDetector(
            behavior: HitTestBehavior.deferToChild,
            onPanStart: (d) {
              setState(() {
                if (_canvasSelectedStrokes.isNotEmpty) {
                  _canvasLassoPts.clear();
                  _canvasDragStart = d.localPosition;
                  _canvasDragNow = d.localPosition;
                } else {
                  _canvasLassoPts
                    ..clear()
                    ..add(d.localPosition);
                }
              });
            },
            onPanUpdate: (d) {
              if (_canvasDragStart != null) {
                setState(() => _canvasDragNow = d.localPosition);
              } else if (_canvasLassoPts.isNotEmpty) {
                setState(() => _canvasLassoPts.add(d.localPosition));
              }
            },
            onPanEnd: (_) {
              if (_canvasDragStart != null) {
                unawaited(_finishCanvasDrag());
              } else {
                unawaited(_finishCanvasLasso());
              }
            },
            onSecondaryTapUp: (d) =>
                _showCanvasEditMenu(d.globalPosition, d.localPosition),
            child: CustomPaint(
              painter: _DetailPainter(
                image: img,
                scale: _canvasEditScale,
                offset: _canvasEditOffset,
                lasso: _canvasLassoPts,
                dragFrom: _canvasDragStart,
                dragTo: _canvasDragStart != null ? _canvasDragNow : null,
                pageRect: Rect.zero,
              ),
              child: const SizedBox.expand(),
            ),
          );
        }),
      ),
    ]);
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
    if (_canvasEditing) return _buildCanvasEditor();
    final body = _canvasLoading
        ? const Center(child: CircularProgressIndicator())
        : (_canvasPng == null
            ? Center(
                child: Text(AppLocalizations.of(context)!.canvasIsEmptyGo, style: TextStyle(fontSize: 14, color: Colors.grey)))
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
                label: Text(AppLocalizations.of(context)!.backToOrigin, style: TextStyle(fontSize: 13)),
              ),
              OutlinedButton.icon(
                onPressed: _canvasLoading ? null : () => _canvasAction('center'),
                icon: const Icon(Icons.center_focus_strong, size: 18),
                label: Text(AppLocalizations.of(context)!.centerContent, style: TextStyle(fontSize: 13)),
              ),
              if (_infiniteCanvas)
                OutlinedButton.icon(
                  onPressed: _canvasLoading ? null : () => _canvasAction('new'),
                  icon: const Icon(Icons.note_add_outlined, size: 18),
                  label: Text(AppLocalizations.of(context)!.newCanvas, style: TextStyle(fontSize: 13)),
                ),
              if (_canvasPng != null && _canvasBounds.length >= 4)
                OutlinedButton.icon(
                  onPressed: _canvasLoading ? null : _openCanvasEditor,
                  icon: const Icon(Icons.edit_outlined, size: 18),
                  label: Text(_canvasText('编辑内容', 'Edit content'),
                      style: const TextStyle(fontSize: 13)),
                ),
              OutlinedButton.icon(
                onPressed: _canvasLoading ? null : () => _loadCanvasOverview(),
                icon: const Icon(Icons.refresh, size: 18),
                label: Text(AppLocalizations.of(context)!.refresh, style: TextStyle(fontSize: 13)),
              ),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(child: body),
        Padding(
          padding: EdgeInsets.all(6),
          child: Text(AppLocalizations.of(context)!.redBoxCurrentMain, style: TextStyle(fontSize: 11, color: Colors.grey)),
        ),
      ],
    );
  }
}
