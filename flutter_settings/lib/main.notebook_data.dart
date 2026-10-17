part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 活页本数据: 页列表/缩略图队列 ──

extension _NotebookDataUi on _SettingsPageState {
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
      if (mounted) {
        setState(() => _pagesLoading = false);
        _toastError(AppLocalizations.of(context)!.loadFailedP1('$e'));
      }
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
}
