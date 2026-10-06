part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 无限画布 tab(自由涂鸦/总览/画布动作) ──

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
}
