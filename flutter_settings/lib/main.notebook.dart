part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 活页本 UI: 网格/详情/圈选/拖拽/删除 + 详情画笔 ──

/// 笔记本封面色: 由尺寸 key 决定(确定性"随机") —— 生成一次终生不变,
/// 任何机器/重装都得到同一本色。平涂色板, GoodNotes 式干净。
const _coverPalette = [
  Color(0xFF4E7DC4), // 钴蓝
  Color(0xFFE06A5E), // 珊瑚红
  Color(0xFF4CA88E), // 青玉
  Color(0xFFE0954E), // 杏橙
  Color(0xFF7B68C4), // 长春花
  Color(0xFFC4588A), // 玫红
  Color(0xFF55A8C4), // 青蓝
  Color(0xFF8FAE4E), // 苔绿
];
Color _coverColor(String key) {
  var hv = 0;
  for (final c in key.codeUnits) {
    hv = (hv * 31 + c) & 0x7FFFFFFF;
  }
  return _coverPalette[hv % _coverPalette.length];
}

extension _NotebookUi on _SettingsPageState {
  /// 笔记本网格: GoodNotes 式平涂封面 —— 纯色 + 右侧**松紧带**(黑色竖带,
  /// 本子的签名元素) + 居中白字标题。**卡片比例 = 该本子的真实比例**
  /// (3440×1440 是胖扁的本子, 竖屏分辨率是瘦高的本子)。
  /// 布局: 列数随窗口宽度自适应、卡片铺满不留缝, 同排卡片**底边对齐**
  /// (书架比喻: 胖瘦本子立在架上)。
  Widget _buildNotebookGrid() {
    final notebooks = _notebooks;
    if (notebooks.isEmpty) {
      return Center(
          child: Text(AppLocalizations.of(context)!.noPagesYet, style: TextStyle(fontSize: 14, color: Colors.grey)));
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
        void openBook() => setState(() {
              _openNotebook = key;
              _multiSelect = false;
              _selectedPageIds.clear();
              _applyPageList();
            });
        if (_notebookGlass) {
          cards.add(SizedBox(
            width: cardW,
            height: cardH,
            child: GestureDetector(
              onTap: openBook,
              child: _glassNotebookCover(
                  key, cardW, cardH, pages.length, cover),
            ),
          ));
          return;
        }
        cards.add(SizedBox(
          width: cardW,
          height: cardH,
          child: GestureDetector(
            onTap: openBook,
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
                        Text(AppLocalizations.of(context)!.p1Pages(pages.length),
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
      _toast(AppLocalizations.of(context)!.pageIsEmptyNothing);
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
  Future<void> _ensureTabBgImage() async {
    if (_tabBgImage != null) return;
    final data = await rootBundle.load('assets/tab_bg_pages.jpg');
    final codec = await ui.instantiateImageCodec(data.buffer.asUint8List());
    final frame = await codec.getNextFrame();
    if (!mounted) return;
    setState(() => _tabBgImage = frame.image);
  }

  Offset _viewToPage(Offset v) =>
      (v - _detailOffset) / _detailScale - Offset(_detailMargin, _detailMargin);

  Future<void> _finishLasso() async {
    if (_lassoPts.length < 3 || _detailPageId == null) {
      setState(() => _lassoPts.clear());
      return;
    }
    final pageId = _detailPageId!;
    final gen = _detailGen;
    final poly = _lassoPts.map(_viewToPage).map((p) => (p.dx, p.dy)).toList();
    final ids = await _bridge.lassoSelect(pageId, poly);
    // 等结果期间关了详情/换了页: 这批 id 属于旧页, 不能落到新页选择上
    // (旧代码这里还会对 _detailPageId! 做非空断言, 关详情时直接抛异常)。
    if (!mounted || gen != _detailGen || _detailPageId != pageId) return;
    setState(() {
      _selectedStrokes = ids;
      _lassoPts.clear();
    });
    if (ids.isEmpty) {
      _toast(AppLocalizations.of(context)!.noStrokesInsideThe);
      unawaited(_reloadDetailImage(pageId));
    } else {
      _toast(AppLocalizations.of(context)!.p1StrokesSelectedBlue(ids.length));
      unawaited(_reloadDetailImage(pageId, highlight: ids));
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
      _toast(AppLocalizations.of(context)!.moveFailed);
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
    _toast(payload.isEmpty ? AppLocalizations.of(context)!.nothingToCopy : AppLocalizations.of(context)!.copiedP1Strokes(_selectedStrokes.length));
  }

  Future<void> _pasteAt(Offset viewPos) async {
    if (_clipPayload.isEmpty || _detailPageId == null) {
      _toast(AppLocalizations.of(context)!.clipboardIsEmpty);
      return;
    }
    final pageId = _detailPageId!;
    final gen = _detailGen;
    final p = _viewToPage(viewPos);
    final n = await _bridge.pasteStrokes(pageId, _clipPayload, p.dx, p.dy);
    if (!mounted) return;
    if (n > 0) {
      _toast(AppLocalizations.of(context)!.pastedP1Strokes(n));
      // 粘贴已落到 pageId; 期间关了/换了详情就不要拿 null/新页去刷新
      if (gen == _detailGen && _detailPageId == pageId) {
        unawaited(_reloadDetailImage(pageId));
      }
    } else {
      _toast(AppLocalizations.of(context)!.pasteFailed);
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
      _toast(AppLocalizations.of(context)!.deletedP1Strokes(ids.length));
    } else {
      _toast(AppLocalizations.of(context)!.deleteFailed);
    }
  }

  Future<void> _undoLastMove() async {
    if (_undoMoves.isEmpty) {
      _toast(AppLocalizations.of(context)!.nothingToUndo);
      return;
    }
    final (pageId, ids, dx, dy) = _undoMoves.removeLast();
    final ok = await _bridge.moveStrokes(pageId, ids, dx, dy);
    if (!mounted) return;
    if (ok && _detailPageId == pageId) {
      unawaited(_reloadDetailImage(pageId));
    }
  }

  void _showDetailMenu(Offset globalPos, Offset canvasPos) {
    final sel = _selectedStrokes.isNotEmpty;
    // showMenu 的 position 相对当前 Overlay。pos 已是全局坐标
    // (TapUpDetails.globalPosition), 只需相对 Overlay 反算; 不要用
    // State 的 context 找 RenderBox(那是整面板, 原点错 → 菜单偏移)。
    // canvasPos 必须用 GestureDetector 的 localPosition: 它与
    // _viewToPage 的 CustomPaint 同一坐标系; 用整面板坐标会把粘贴点
    // 按顶部 tab/工具条的高度往下偏(用户看到的"粘错位置")。
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
          PopupMenuItem(
              value: 'copy', height: 36,
              child: Row(children: [
                Icon(Icons.copy, size: 16),
                SizedBox(width: 8),
                Text(AppLocalizations.of(context)!.copySelection, style: TextStyle(fontSize: 13)),
              ])),
        if (_clipPayload.isNotEmpty)
          PopupMenuItem(
              value: 'paste', height: 36,
              child: Row(children: [
                Icon(Icons.paste, size: 16),
                SizedBox(width: 8),
                Text(AppLocalizations.of(context)!.pasteHere, style: TextStyle(fontSize: 13)),
              ])),
        if (sel)
          PopupMenuItem(
              value: 'delete', height: 36,
              child: Row(children: [
                Icon(Icons.delete_outline, size: 16, color: Colors.red),
                SizedBox(width: 8),
                Text(AppLocalizations.of(context)!.deleteSelection, style: TextStyle(fontSize: 13)),
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
              label: Text(AppLocalizations.of(context)!.notebooks, style: TextStyle(fontSize: 13)),
            ),
            const SizedBox(width: 8),
            Text(AppLocalizations.of(context)!.pageP1P2Strokes(_detailPageId ?? 0, _selectedStrokes.length),
                style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
            const Spacer(),
            TextButton.icon(
              onPressed: _copySelection,
              icon: const Icon(Icons.copy, size: 16),
              label: Text(AppLocalizations.of(context)!.copy, style: TextStyle(fontSize: 13)),
            ),
            TextButton.icon(
              onPressed: _deleteSelection,
              icon: const Icon(Icons.delete_outline, size: 16),
              label: Text(AppLocalizations.of(context)!.deleteSelection, style: TextStyle(fontSize: 13)),
            ),
            TextButton.icon(
              onPressed: _undoLastMove,
              icon: const Icon(Icons.undo, size: 16),
              label: Text(AppLocalizations.of(context)!.undoMove, style: TextStyle(fontSize: 13)),
            ),
          ]),
        ),
        Padding(
          padding: EdgeInsets.symmetric(horizontal: 12),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(AppLocalizations.of(context)!.dragLassoSelectDrag,
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
            onSecondaryTapUp: (d) =>
                _showDetailMenu(d.globalPosition, d.localPosition),
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
    // 真穿透+拟物: 确保插画位图已加载(加载完成经 setState 刷新)
    if (_notebookGlass && _panelTransparent) unawaited(_ensureTabBgImage());
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
                label: Text(AppLocalizations.of(context)!.notebooks, style: TextStyle(fontSize: 13)),
              ),
              const SizedBox(width: 4),
              if (_searchMode) ...[
                Text(AppLocalizations.of(context)!.searchP1(_searchText),
                    style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600)),
                const SizedBox(width: 6),
                IconButton(
                  tooltip: AppLocalizations.of(context)!.clearSearch,
                  onPressed: _clearSearch,
                  icon: const Icon(Icons.close, size: 16),
                ),
              ] else ...[
                Text(key.replaceAll('x', ' × '),
                    style: const TextStyle(
                        fontSize: 14, fontWeight: FontWeight.w600)),
                const SizedBox(width: 8),
              ],
              Text(AppLocalizations.of(context)!.p1Pages(_filteredPages.length),
                  style: const TextStyle(fontSize: 12, color: _inkFaint)),
              const Spacer(),
              if (_searchFieldVisible)
                SizedBox(
                  width: 200,
                  height: 30,
                  child: TextField(
                    autofocus: true,
                    style: const TextStyle(fontSize: 13),
                    decoration: InputDecoration(
                      hintText: AppLocalizations.of(context)!.searchTextInPages,
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
                  tooltip: AppLocalizations.of(context)!.ocrSearch,
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
                          ? AppLocalizations.of(context)!.noMatchingPagesWas
                          : AppLocalizations.of(context)!.theShelfIsEmpty,
                      style:
                          const TextStyle(fontSize: 14, color: Colors.grey)))
              : Stack(children: [
                  // 真穿透+拟物: 插画铺满网格底层, 页卡位置挖洞透桌面
                  if (_notebookGlass &&
                      _panelTransparent &&
                      _tabBgImage != null)
                    Positioned.fill(
                      child: CustomPaint(
                        painter: _TabIllustrationPainter(_tabBgImage!),
                      ),
                    ),
                  GridView.builder(
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
                    // 真穿透+拟物: 卡片自带抠洞层(BlendMode.clear 擦掉
                    // 底层插画), 洞内透出面板后的真实桌面。
                    final hole = _notebookGlass &&
                        _panelTransparent &&
                        _tabBgImage != null;
                    // 长按拖动换位置(iOS 弹簧桌式): 悬停别的卡片时本地
                    // 列表实时移位(其他页让出), 松手一次性提交 DB。
                    // ValueKey 保活手势元素: 列表移位时拖拽不中断。
                    return Stack(children: [
                      if (hole)
                        const Positioned.fill(
                            child: CustomPaint(painter: _HolePainter())),
                      LongPressDraggable<int>(
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
                      ), // LongPressDraggable 收尾
                    ]);
                  }
                ),
          ],
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
    final anchor = pageDragCommitAnchor(_filteredPages, dragged);
    if (anchor == null) return;
    final (anchorId, before) = anchor;
    final ok = await _bridge.reorderPage(dragged, anchorId, before: before);
    if (!mounted) return;
    if (ok) {
      unawaited(_loadPages()); // 以 DB 为准刷新(缩略图缓存未失效, 很快)
    } else {
      setState(() {
        _filteredPages = List<PageInfo>.from(_dragOrderBackup);
      });
      _toast(AppLocalizations.of(context)!.moveFailedReverted);
    }
  }


  /// 活页本网格工具条:常态 = 页数 + 「多选」入口;
  /// 多选态 = 已选计数 + 全选 / 删除所选 / 取消。
  Widget _buildGridToolbar() {
    const faint = TextStyle(fontSize: 12, color: _inkFaint);
    if (!_multiSelect) {
      return Row(children: [
        Text(AppLocalizations.of(context)!.p1Pages(_filteredPages.length), style: faint),
        const Spacer(),
        TextButton.icon(
          onPressed: () => setState(() {
            _multiSelect = true;
            _selectedPageIds.clear();
          }),
          icon: const Icon(Icons.checklist, size: 16),
          label: Text(AppLocalizations.of(context)!.multiSelect, style: TextStyle(fontSize: 13)),
        ),
      ]);
    }
    return Row(children: [
      Text(AppLocalizations.of(context)!.p1PagesSelected(_selectedPageIds.length), style: faint),
      const Spacer(),
      TextButton.icon(
        onPressed: (_batchDeleting || _selectedPageIds.isEmpty) ? null : () async {
          setState(() => _batchDeleting = true);
          try {
            final ok = await _bridge.exportSelectedPdf(_selectedPageIds.toList());
            if (mounted) {
              _toast(ok ? AppLocalizations.of(context)!.exportedP1PagesAs(_selectedPageIds.length) : AppLocalizations.of(context)!.exportFailed);
            }
          } finally {
            if (mounted) setState(() => _batchDeleting = false);
          }
        },
        style: TextButton.styleFrom(foregroundColor: _ink),
        icon: const Icon(Icons.picture_as_pdf, size: 16),
        label: Text(AppLocalizations.of(context)!.exportPdf, style: TextStyle(fontSize: 13)),
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
            ? AppLocalizations.of(context)!.deselectAll : AppLocalizations.of(context)!.selectAll, style: TextStyle(fontSize: 13)),
      ),
      TextButton(
        onPressed: (_batchDeleting || _selectedPageIds.isEmpty) ? null : _confirmBatchDelete,
        style: TextButton.styleFrom(foregroundColor: Colors.red),
        child: Text(_batchDeleting ? AppLocalizations.of(context)!.deleting : AppLocalizations.of(context)!.deleteSelectedP1(_selectedPageIds.length),
            style: const TextStyle(fontSize: 13)),
      ),
      TextButton(
        onPressed: () => setState(() {
          _multiSelect = false;
          _selectedPageIds.clear();
        }),
        child: Text(AppLocalizations.of(context)!.cancel, style: TextStyle(fontSize: 13)),
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
        title: Text(AppLocalizations.of(context)!.batchDelete),
        content: Text(AppLocalizations.of(context)!.deleteTheP1Selected(ids.length)),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: Text(AppLocalizations.of(context)!.cancel)),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: TextButton.styleFrom(foregroundColor: Colors.red),
            child: Text(AppLocalizations.of(context)!.deleteP1Pages(ids.length))
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
      SnackBar(content: Text(AppLocalizations.of(context)!.deletedP1Pages(ok)), duration: Duration(seconds: 2)),
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
  List<PageInfo> _groupSorted(List<PageInfo> pages) =>
      groupSortedPages(pages); // 纯函数在 main.models.dart(可单测)

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

    final inner = Stack(children: [
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
            child: Text(AppLocalizations.of(context)!.pageP1(page.id),
                style: const TextStyle(
                    fontWeight: FontWeight.bold, fontSize: 13)),
          ),
        ],
      ),
      // 多选角标
      if (_multiSelect)
        Positioned(
          top: 6, right: 6,
          child: Icon(
            selected ? Icons.check_circle : Icons.radio_button_unchecked,
            size: 22,
            color: selected ? _penRed : Colors.white,
            shadows: const [Shadow(blurRadius: 4, color: Colors.black38)],
          ),
        ),
    ]);

    final ink = InkWell(
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
                  PopupMenuItem(
                      value: 'detail', height: 36,
                      child: Row(children: [
                        Icon(Icons.draw_outlined, size: 16),
                        SizedBox(width: 8),
                        Text(AppLocalizations.of(context)!.editContentLassoMove, style: TextStyle(fontSize: 13)),
                      ])),
                  PopupMenuItem(
                      value: 'png', height: 36,
                      child: Row(children: [
                        Icon(Icons.image_outlined, size: 16),
                        SizedBox(width: 8),
                        Text(AppLocalizations.of(context)!.exportThisPageAs, style: TextStyle(fontSize: 13)),
                      ])),
                  PopupMenuItem(
                      value: 'svg', height: 36,
                      child: Row(children: [
                        Icon(Icons.polyline_outlined, size: 16),
                        SizedBox(width: 8),
                        Text(AppLocalizations.of(context)!.exportThisPageAs2, style: TextStyle(fontSize: 13)),
                      ])),
                  PopupMenuItem(
                      value: 'info', height: 36,
                      child: Row(children: [
                        Icon(Icons.info_outline, size: 16),
                        SizedBox(width: 8),
                        Text(AppLocalizations.of(context)!.copyPageInfo, style: TextStyle(fontSize: 13)),
                      ])),
                  const PopupMenuDivider(),
                  PopupMenuItem(
                      value: 'delete', height: 36,
                      child: Row(children: [
                        Icon(Icons.delete_outline, size: 16, color: Colors.red),
                        SizedBox(width: 8),
                        Text(AppLocalizations.of(context)!.deleteThisPage, style: TextStyle(fontSize: 13)),
                      ])),
                ],
              ).then((v) async {
                if (!mounted) return;
                if (v == 'detail') {
                  await _openPageDetail(page.id);
                } else if (v == 'delete') {
                  _confirmDeletePage(page);
                } else if (v == 'png') {
                  final l = AppLocalizations.of(context)!;
                  final ok = await _bridge.exportPagePng(page.id);
                  _toast(ok ? l.pngExportedToDesktop : l.exportFailedEmptyPage);
                } else if (v == 'svg') {
                  final l = AppLocalizations.of(context)!;
                  final ok = await _bridge.exportPageSvg(page.id);
                  _toast(ok ? l.svgExportedToDesktop : l.exportFailedEmptyPage);
                } else if (v == 'info') {
                  final l = AppLocalizations.of(context)!;
                  final t = DateTime.fromMillisecondsSinceEpoch(page.id * 1000);
                  Clipboard.setData(ClipboardData(text:
                      l.pageP1SizeP2(page.id, page.w, page.h, page.strokeCount) +
                      l.createdP1('${t.year}-${t.month.toString().padLeft(2, '0')}-${t.day.toString().padLeft(2, '0')}')));
                  _toast(AppLocalizations.of(context)!.pageInfoCopied);
                }
              });
            },
      onTap: () {
        if (_multiSelect) {
          setState(() => selected
              ? _selectedPageIds.remove(page.id)
              : _selectedPageIds.add(page.id));
          return;
        }
        _bridge.navigateToPage(page.id);
      },
      child: inner,
    );

    return GestureDetector(
      // 拟物玻璃: 磨砂卡(BackdropFilter 模糊背后桌布 + 斜向高光 + 玻璃边);
      // 笔记纸: 原 Card。
      child: _notebookGlass
          ? _glassPageWrap(ink, selected: selected)
          : Card(
              clipBehavior: Clip.antiAlias,
              shape: _multiSelect && selected
                  ? RoundedRectangleBorder(
                      side: const BorderSide(color: _penRed, width: 2),
                      borderRadius: BorderRadius.circular(10))
                  : null,
              child: ink,
            ),
    );
  }

  /// 磨砂玻璃卡装饰: ClipRRect + BackdropFilter(模糊背后桌布) + 斜向
  /// 高光渐变 + 玻璃白边; 选中 = 红边加粗。child 铺满。
  Widget _glassPageWrap(Widget child, {required bool selected}) {
    // 真穿透(实验): 面板窗口可透明时卡片体不画任何填充/模糊
    // (alpha=0 = 透出面板后的桌面), 只留玻璃边 —— 清玻璃形态。
    if (!_panelTransparent) {
      return ClipRRect(
        borderRadius: BorderRadius.circular(14),
        child: BackdropFilter(
          filter: ui.ImageFilter.blur(sigmaX: 7, sigmaY: 7),
          child: Container(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  Colors.white.withValues(alpha: 0.50),
                  Colors.white.withValues(alpha: 0.20),
                ],
              ),
              border: Border.all(
                color: selected
                    ? _penRed
                    : Colors.white.withValues(alpha: 0.65),
                width: selected ? 2 : 1,
              ),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.25),
                  blurRadius: 12,
                  offset: const Offset(3, 5),
                ),
              ],
            ),
            child: Stack(children: [
              child,
              // 斜向高光(玻璃反光)—— 仅模糊形态
              Positioned.fill(
                child: IgnorePointer(
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.topLeft,
                        end: Alignment.bottomRight,
                        stops: const [0, 0.12, 0.45],
                        colors: [
                          Colors.white.withValues(alpha: 0.30),
                          Colors.white.withValues(alpha: 0.05),
                          Colors.transparent,
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ]),
          ),
        ),
      );
    }
    // 清玻璃: 无填充无高光(0.30 白高光在透窗形态下就是白雾),
    // 只留玻璃边 —— 墨迹直接浮在真实桌面上。
    return ClipRRect(
      borderRadius: BorderRadius.circular(14),
      child: Container(
        decoration: BoxDecoration(
          border: Border.all(
            color: selected ? _penRed : Colors.white.withValues(alpha: 0.55),
            width: selected ? 2 : 1,
          ),
        ),
        child: child,
      ),
    );
  }


  /// 拟物本子封面: 同尺寸玻璃叠层 —— 层数示意页数(封顶 6 层),
  /// 纵横比 = 本子分辨率, 角标给精确页数。
  Widget _glassNotebookCover(String key, double cardW, double cardH,
      int pageCount, Color coverFallback) {
    final parts = key.split('x');
    final w = parts[0], h = parts[1];
    final layers = (pageCount - 1).clamp(0, 6);
    return Stack(children: [
      for (int i = layers; i >= 1; i--)
        Positioned(
          left: 0, top: 0,
          right: i * 5.0, bottom: i * 5.0,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(14),
            child: BackdropFilter(
              filter: ui.ImageFilter.blur(sigmaX: 5, sigmaY: 5),
              child: Container(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topLeft,
                    end: Alignment.bottomRight,
                    colors: [
                      Colors.white.withValues(alpha: 0.34),
                      Colors.white.withValues(alpha: 0.14),
                    ],
                  ),
                  border: Border.all(
                      color: Colors.white.withValues(alpha: 0.45)),
                ),
              ),
            ),
          ),
        ),
      Positioned.fill(
        child: _glassPageWrap(
          Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('$w × $h',
                    style: TextStyle(
                        fontSize: (cardW * 0.055).clamp(13.0, 17.0),
                        fontWeight: FontWeight.w700,
                        color: Colors.black87)),
                const SizedBox(height: 4),
                Text(AppLocalizations.of(context)!.p1Pages(pageCount),
                    style: TextStyle(
                        fontSize: (cardW * 0.05).clamp(12.0, 15.0),
                        color: Colors.black54)),
                const Spacer(),
                Align(
                  alignment: Alignment.bottomRight,
                  child: Icon(Icons.layers,
                      size: (cardW * 0.05).clamp(14.0, 20.0),
                      color: Colors.black26),
                ),
              ],
            ),
          ),
          selected: false,
        ),
      ),
    ]);
  }
  void _confirmDeletePage(PageInfo page) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(AppLocalizations.of(context)!.deletePage),
        content: Text(AppLocalizations.of(context)!.deletePageP1And(page.id)),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(), child: Text(AppLocalizations.of(context)!.cancel)),
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              _deletePage(page);
            },
            style: TextButton.styleFrom(foregroundColor: Colors.red),
            child: Text(AppLocalizations.of(context)!.delete),
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
            SnackBar(content: Text(AppLocalizations.of(context)!.deleteFailed), duration: Duration(seconds: 2)),
          );
        }
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(AppLocalizations.of(context)!.deleteFailedP1(e))),
        );
      }
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


/// 真穿透插画面层: tab_bg_pages 铺满网格区域(slice cover, 85% 白纱调和)。
class _TabIllustrationPainter extends CustomPainter {
  _TabIllustrationPainter(this.image);
  final ui.Image image;

  @override
  void paint(Canvas canvas, Size size) {
    final kx = size.width / image.width;
    final ky = size.height / image.height;
    final k = kx > ky ? kx : ky; // cover
    final w = image.width * k, h = image.height * k;
    final dst = Rect.fromLTWH(
        (size.width - w) / 2, (size.height - h) / 2, w, h);
    canvas.drawImageRect(
      image,
      Rect.fromLTWH(0, 0, image.width.toDouble(), image.height.toDouble()),
      dst,
      Paint()
        ..filterQuality = FilterQuality.medium
        ..color = const Color(0xD9FFFFFF),
    );
  }

  @override
  bool shouldRepaint(_TabIllustrationPainter old) => old.image != image;
}

/// 卡片挖洞层(isBackground: 画在卡片内容之前): BlendMode.clear 把插画
/// 在卡片矩形内擦成透明 —— 洞里透出面板后的真实桌面。须与插画同一
/// 画布层级(无 RepaintBoundary 隔离)才能擦到。
class _HolePainter extends CustomPainter {
  const _HolePainter();

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRRect(
      RRect.fromRectAndRadius(Offset.zero & size, const Radius.circular(12)),
      Paint()..blendMode = BlendMode.clear,
    );
  }

  @override
  bool shouldRepaint(_HolePainter old) => false;
}
