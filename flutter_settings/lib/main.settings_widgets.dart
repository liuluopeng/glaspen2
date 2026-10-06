part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 设置页部件: 颜色/宽度/GIF/热键/开关/备份/导出 ──

extension _SettingsWidgetsUi on _SettingsPageState {
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
        chips<int>(AppLocalizations.of(context)!.frameRate, fpsOptions, _gifFps, (v) => '$v fps', (v) {
          _gifFps = v;
          _setSetting('gifFps', v);
        }),
        const SizedBox(height: 8),
        chips<double>(AppLocalizations.of(context)!.resolution, resOptions, _gifResolution, (v) => '${(v * 100).round()}%',
            (v) {
          _gifResolution = v;
          _setSetting('gifResolution', v);
        }),
        const SizedBox(height: 8),
        chips<double>(AppLocalizations.of(context)!.speed, speedOptions, _gifSpeed, (v) => '${v}x', (v) {
          _gifSpeed = v;
          _setSetting('gifSpeed', v);
        }),
        const SizedBox(height: 8),
        chips<int>(AppLocalizations.of(context)!.ending, [0, 1, 2], _gifEndMode,
            (v) => v == 0
                ? AppLocalizations.of(context)!.stopOnLastFrame
                : v == 1
                    ? AppLocalizations.of(context)!.hold1SThen
                    : AppLocalizations.of(context)!.loopImmediately, (v) {
          _gifEndMode = v;
          _setSetting('gifEndMode', v);
        }),
        const SizedBox(height: 10),
        Text(AppLocalizations.of(context)!.estimatedSizeP1(_estimateGifSize()),
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
        row([key('Q', AppLocalizations.of(context)!.quitApp)]),
        row([key('G', AppLocalizations.of(context)!.exportImage), key('`', AppLocalizations.of(context)!.previousPage), key('1', AppLocalizations.of(context)!.nextPage)], indent: 22),
        row([
          key('Z', AppLocalizations.of(context)!.undoLastStroke),
          key('X', AppLocalizations.of(context)!.etherealCanvas),
          key('C', AppLocalizations.of(context)!.newCanvas),
          key('V', AppLocalizations.of(context)!.toggleDoodle),
          key('B', AppLocalizations.of(context)!.blurBackground),
        ]),
      ],
    );
  }

  // ── 笔:颜色 + 粗细 ──
  Widget _buildPenSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(AppLocalizations.of(context)!.color,
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 8),
        _buildColorGrid(),
        const SizedBox(height: 14),
        Text(AppLocalizations.of(context)!.width,
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
        Text(Platform.isMacOS ? AppLocalizations.of(context)!.gifAnimationRTo : AppLocalizations.of(context)!.gifAnimationHoldCtrl,
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        _buildGifSettings(),
        const Divider(height: 24),
        if (Platform.isMacOS) ...[
          Row(
            children: [
              Text(AppLocalizations.of(context)!.pageFlipEffect,
                  style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
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
          title: Text(AppLocalizations.of(context)!.freeDoodleMode, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.showTheFreeDoodle,
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
          title: Text(AppLocalizations.of(context)!.pressureMonitor, style: TextStyle(fontSize: 15)),
          value: _pressureMonitor,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _pressureMonitor = v);
            _setSetting('pressureMonitor', v);
          },
        )),
        _tile(SwitchListTile(
          title: Text(AppLocalizations.of(context)!.showGrid, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.helpsAlignWhileDoodling, style: TextStyle(fontSize: 12)),
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
                Text(AppLocalizations.of(context)!.gridSize, style: TextStyle(fontSize: 13, color: _inkFaint)),
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
                Text(AppLocalizations.of(context)!.columns, style: TextStyle(fontSize: 13, color: _inkFaint)),
                const SizedBox(height: 4),
                SizedBox(
                  width: double.infinity,
                  child: SegmentedButton<int>(
                    showSelectedIcon: false,
                    style: const ButtonStyle(
                      visualDensity: VisualDensity(horizontal: -2, vertical: -2),
                    ),
                    segments: [
                      ButtonSegment(value: 0, label: Text(AppLocalizations.of(context)!.none)),
                      ButtonSegment(value: 1, label: Text(AppLocalizations.of(context)!.twoColumnsSideBy)),
                      ButtonSegment(value: 2, label: Text(AppLocalizations.of(context)!.twoColumnsStacked)),
                      ButtonSegment(value: 3, label: Text(AppLocalizations.of(context)!.grid3x3)),
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
          title: Text(AppLocalizations.of(context)!.gridFollowsDoodles, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.whenOnTheGrid, style: TextStyle(fontSize: 12)),
          value: _gridFollowStrokes,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _gridFollowStrokes = v);
            _setSetting('gridFollowStrokes', v);
          },
        )),
        _tile(SwitchListTile(
          title: Text(AppLocalizations.of(context)!.glassFollowsDoodles, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.whenOnTheGlass, style: TextStyle(fontSize: 12)),
          value: _glassFollowStrokes,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _glassFollowStrokes = v);
            _setSetting('glassFollowStrokes', v);
          },
        )),
        _tile(SwitchListTile(
          title: Text(AppLocalizations.of(context)!.inkOutline, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.dashedContrastOutlineVisible, style: TextStyle(fontSize: 12)),
          value: _outlineEnabled,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _outlineEnabled = v);
            _setSetting('outline', v);
          },
        )),
        _tile(SwitchListTile(
          title: Text(AppLocalizations.of(context)!.softShadow, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.softDarkShadowUnder, style: TextStyle(fontSize: 12)),
          value: _softShadow,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _softShadow = v);
            _setSetting('softShadow', v);
          },
        )),
        _tile(SwitchListTile(
          title: Text(AppLocalizations.of(context)!.invertedInk, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.strokesTakeThePer, style: TextStyle(fontSize: 12)),
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
        _tile(Padding(
          padding: EdgeInsets.only(left: 4, bottom: 8),
          child: Text(AppLocalizations.of(context)!.invertTrackingFpsCap, style: TextStyle(fontSize: 11, color: Colors.grey)),
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
        Text(
          AppLocalizations.of(context)!.exportAllPagesStrokes,
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
          label: Text(_pdfExporting ? AppLocalizations.of(context)!.exporting : AppLocalizations.of(context)!.exportAllPagesAs),
          onPressed: _pdfExporting ? null : _exportPdf,
        ),
      ],
    );
  }


  Widget _buildBackupButtons() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          AppLocalizations.of(context)!.backupIsOneDb +
          AppLocalizations.of(context)!.restoreMergesItNever,
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
              label: Text(_backupBusy ? AppLocalizations.of(context)!.working : AppLocalizations.of(context)!.backUpAllData),
              onPressed: _backupBusy ? null : _backupNow,
            ),
            const SizedBox(width: 8),
            OutlinedButton.icon(
              icon: const Icon(Icons.settings_backup_restore, size: 18),
              label: Text(AppLocalizations.of(context)!.restoreFromLatestBackup),
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
      _toast(ok ? AppLocalizations.of(context)!.backedUpToP1(message) : AppLocalizations.of(context)!.backupFailedP1(message));
    } catch (e) {
      if (mounted) _toast(AppLocalizations.of(context)!.backupFailedP1(e));
    } finally {
      if (mounted) setState(() => _backupBusy = false);
    }
  }

  Future<void> _confirmRestore() async {
    final yes = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(AppLocalizations.of(context)!.restoreFromLatestBackup),
        content: Text(
            AppLocalizations.of(context)!.mergesTheNewestGlaspen2 +
            AppLocalizations.of(context)!.pagesStrokesAlsoPresent),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: Text(AppLocalizations.of(context)!.cancel)),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(AppLocalizations.of(context)!.restore),
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
          title: Text(AppLocalizations.of(context)!.restoreFromLatestBackup, style: TextStyle(fontSize: 16)),
          content: Text(AppLocalizations.of(context)!.pagesFromTheBackup, style: TextStyle(fontSize: 13)),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(ctx, false),
                child: Text(AppLocalizations.of(context)!.cancel)),
            TextButton(
                onPressed: () => Navigator.pop(ctx, true),
                child: Text(AppLocalizations.of(context)!.restore)),
          ],
        ),
      );
      if (confirmed != true || !mounted) return;
      final (ok, message) = await _bridge.restoreLatestBackup();
      if (!mounted) return;
      _toast(ok ? AppLocalizations.of(context)!.restoredFromBackupP1(message) : AppLocalizations.of(context)!.restoreFailedP1(message));
      if (ok) {
        // 缩略图缓存与列表都可能过期, 清掉重拉
        _thumbnailCache.clear();
        _pages = [];
        _filteredPages = [];
        _loadPages();
      }
    } catch (e) {
      if (mounted) _toast(AppLocalizations.of(context)!.restoreFailedP1(e));
    } finally {
      if (mounted) setState(() => _backupBusy = false);
    }
  }


  Future<void> _exportPdf() async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(AppLocalizations.of(context)!.exportAllPages, style: TextStyle(fontSize: 16)),
        content: Text(AppLocalizations.of(context)!.allPagesMergeInto,
            style: TextStyle(fontSize: 13)),
        actions: [
          TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: Text(AppLocalizations.of(context)!.cancel)),
          TextButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: Text(AppLocalizations.of(context)!.export)),
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
            content: Text(ok ? AppLocalizations.of(context)!.pdfSavedToDesktop : AppLocalizations.of(context)!.exportFailed),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(AppLocalizations.of(context)!.pdfExportFailedP1(e))),
        );
      }
    } finally {
      if (mounted) setState(() => _pdfExporting = false);
    }
  }
}
