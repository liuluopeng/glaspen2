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
    final names = _colorNamesOf(AppLocalizations.of(context)!);
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: List.generate(names.length, (i) {
        final selected = i == _selectedColor;
        // 用真实亮度决定文字颜色: 此前用"最后一个索引"当白色,
        // 而最后一个预设其实是黑色 —— 黑块黑字、白块白字都看不见。
        final swatch = Color(_colorValues[i]);
        final useDarkText = swatch.computeLuminance() > 0.55;
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
              names[i],
              style: TextStyle(
                fontSize: 13,
                color: useDarkText ? Colors.black87 : Colors.white,
                fontWeight: selected ? FontWeight.bold : FontWeight.normal,
              ),
            ),
          ),
        );
      }),
    );
  }

  Widget _buildWidthRow() {
    final names = _widthNamesOf(AppLocalizations.of(context)!);
    return Wrap(
      spacing: 8,
      children: List.generate(names.length, (i) {
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
            child: Text(names[i],
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

  /// 卡片(_buildSection 的 DecoratedBox)里直接放 ListTile 会触发框架断言
  /// ("background color or ink splashes may be invisible"),包一层透明 Material。
  Widget _tile(Widget child) {
    return Material(type: MaterialType.transparency, child: child);
  }

  // ── 画布外观: 自由涂鸦 tab 显隐 + 网格 + 跟随 + 笔迹风格(描边/阴影/反色) ──
  Widget _buildAppearanceSection() {
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
          title: Text(AppLocalizations.of(context)!.bubbleTab, style: TextStyle(fontSize: 15)),
          subtitle: Text(AppLocalizations.of(context)!.showBubbleTabSub, style: TextStyle(fontSize: 12)),
          value: _showBubbleTab,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() {
              _showBubbleTab = v;
              _syncTabCount();
            });
            _setSetting('showBubbleTab', v);
          },
        )),
        // 气泡重播设置(嵌套在气泡 tab 开关下; 纯面板自用, 三处同步)
        if (_showBubbleTab)
          Padding(
            padding: const EdgeInsets.only(left: 16, bottom: 4),
            child: Row(
              children: [
                Text(AppLocalizations.of(context)!.bubbleReplaySpeed, style: TextStyle(fontSize: 13, color: _inkFaint)),
                const Spacer(),
                SegmentedButton<int>(
                  showSelectedIcon: false,
                  style: const ButtonStyle(
                    visualDensity: VisualDensity(horizontal: -2, vertical: -2),
                  ),
                  segments: [
                    ...[1, 2, 4].map((v) => ButtonSegment(value: v, label: Text('$v\u00d7'))),
                  ],
                  selected: {_bubbleReplaySpeed},
                  onSelectionChanged: (s) {
                    setState(() => _bubbleReplaySpeed = s.first);
                    _setSetting('bubbleReplaySpeed', s.first);
                  },
                ),
              ],
            ),
          ),
        if (_showBubbleTab)
          _tile(SwitchListTile(
            title: Text(AppLocalizations.of(context)!.bubbleReplayHoldLast, style: TextStyle(fontSize: 15)),
            subtitle: Text(AppLocalizations.of(context)!.bubbleReplayHoldLastSub, style: TextStyle(fontSize: 12)),
            value: _bubbleReplayHoldLast,
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) {
              setState(() => _bubbleReplayHoldLast = v);
              _setSetting('bubbleReplayHoldLast', v);
            },
          )),
        if (_showBubbleTab)
          _tile(SwitchListTile(
            title: Text(AppLocalizations.of(context)!.bubbleReplayAuto, style: TextStyle(fontSize: 15)),
            subtitle: Text(AppLocalizations.of(context)!.bubbleReplayAutoSub, style: TextStyle(fontSize: 12)),
            value: _bubbleReplayAuto,
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) {
              setState(() => _bubbleReplayAuto = v);
              _setSetting('bubbleReplayAuto', v);
            },
          )),
        if (_showBubbleTab)
          _tile(SwitchListTile(
            title: Text(AppLocalizations.of(context)!.bubbleAutoNewPage, style: TextStyle(fontSize: 15)),
            subtitle: Text(AppLocalizations.of(context)!.bubbleAutoNewPageSub, style: TextStyle(fontSize: 12)),
            value: _bubbleAutoNewPage,
            dense: true,
            contentPadding: EdgeInsets.zero,
            onChanged: (v) {
              setState(() => _bubbleAutoNewPage = v);
              _setSetting('bubbleAutoNewPage', v);
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
        // 分栏独立于「显示网格」: 两者并列, 各自控制(关网格仍可单独分栏)
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
                    ButtonSegment(value: 4, label: Text(AppLocalizations.of(context)!.cornell)),
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
        // 帧率上限只在反色开启时有意义: 关着时不再占一整行
        if (_invertInk) ...[
          _tile(Padding(
            padding: const EdgeInsets.only(left: 16, top: 4),
            child: SizedBox(
              width: double.infinity,
              child: SegmentedButton<int>(
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
              ),
            ),
          )),
          _tile(Padding(
            padding: EdgeInsets.only(left: 20, bottom: 8),
            child: Text(AppLocalizations.of(context)!.invertTrackingFpsCap, style: TextStyle(fontSize: 11, color: Colors.grey)),
          )),
        ],
        // 页面缩略图(minimap)开关已移到「活页本」tab
        // 无限画布模式由 tab 承载:活页本=翻页模式,自由涂鸦=无限画布
        // (选中 tab 的红线下方,模式 tab 上有一颗小圆点)
      ],
    );
  }

  // ── 系统与辅助: 压力监控 + 开机自启 ──
  Widget _buildSystemSection() {
    return Column(
      children: [
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
          title: Text(AppLocalizations.of(context)!.launchAtLogin, style: TextStyle(fontSize: 15)),
          value: _launchAtLogin,
          dense: true,
          contentPadding: EdgeInsets.zero,
          onChanged: (v) {
            setState(() => _launchAtLogin = v);
            _setSetting('launchAtLogin', v);
          },
        )),
      ],
    );
  }

  // ── 录制与导出: GIF 参数 + 整库 PDF 导出 ──
  Widget _buildRecordExportSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(Platform.isMacOS ? AppLocalizations.of(context)!.gifAnimationRTo : AppLocalizations.of(context)!.gifAnimationHoldCtrl,
            style: TextStyle(fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600)),
        const SizedBox(height: 10),
        _buildGifSettings(),
        const Divider(height: 24),
        _buildExportButtons(),
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
        const SizedBox(height: 14),
        // NAS 备份: 整库快照推送到 NAS 的 axum(文件落盘 + Postgres 登记)
        Text(
          AppLocalizations.of(context)!.nasBackup,
          style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 4),
        Text(
          AppLocalizations.of(context)!.nasBackupSub,
          style: TextStyle(fontSize: 12, color: Colors.grey),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _nasUrl,
          style: const TextStyle(fontSize: 13),
          keyboardType: TextInputType.url,
          decoration: InputDecoration(
            isDense: true,
            hintText: 'http://192.168.1.10:8080',
            border: const OutlineInputBorder(),
            contentPadding:
                const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            labelText: AppLocalizations.of(context)!.nasBackupUrl,
          ),
          onSubmitted: (v) => _setSetting('nasUrl', v.trim()),
        ),
        const SizedBox(height: 8),
        FilledButton.icon(
          icon: _nasBusy
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                      strokeWidth: 2, color: Colors.white),
                )
              : const Icon(Icons.cloud_upload, size: 18),
          label: Text(AppLocalizations.of(context)!.nasBackupNow),
          onPressed: _nasBusy ? null : _nasBackupNow,
        ),
      ],
    );
  }

  /// NAS 备份: 先落地址(输入框可能还没提交), 再跑整库快照 + 上传。
  Future<void> _nasBackupNow() async {
    _setSetting('nasUrl', _nasUrl.text.trim());
    setState(() => _nasBusy = true);
    try {
      final (ok, message) = await _bridge.nasBackupNow();
      if (!mounted) return;
      _toast(ok
          ? '${AppLocalizations.of(context)!.nasBackupDone}: $message'
          : '${AppLocalizations.of(context)!.nasBackupFailed}: $message');
    } catch (e) {
      if (mounted) {
        _toast('${AppLocalizations.of(context)!.nasBackupFailed}: $e');
      }
    } finally {
      if (mounted) setState(() => _nasBusy = false);
    }
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

  // ── 气泡消息 tab: 手写涂鸦按停顿分段, 聊天窗样式回顾(右侧 = 用户口吻) ──

  Future<void> _loadBubbles() async {
    _stopReplay(); // 数据要换了, 旧重播作废
    setState(() => _bubblesLoading = true);
    try {
      // 先把进行中的段结为消息, 再拉全部
      await _bridge.flushBubbles();
      final s = await _bridge.getBubbles();
      if (!mounted) return;
      final v = jsonDecode(s) as Map<String, dynamic>;
      setState(() =>
          _bubbleMsgs = (v['messages'] as List).cast<Map<String, dynamic>>());
      // 自动重播: 进 tab(或手动刷新)后最新一条自动写一遍
      if (_bubbleReplayAuto && _bubbleMsgs.isNotEmpty) {
        _startReplay(_bubbleMsgs.last);
      }
      // 新内容后滚到最底(最新消息)
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_bubbleScroll.hasClients) {
          _bubbleScroll.jumpTo(_bubbleScroll.position.maxScrollExtent);
        }
      });
    } catch (e) {
      if (mounted) setState(() => _bubbleMsgs = const []);
    } finally {
      if (mounted) setState(() => _bubblesLoading = false);
    }
  }

  /// 重播一条消息: 时间线 = 笔内弧长匀速 + 笔间真实停顿(g 字段), 总时长
  /// 除以速度倍率。再点一次同一条 = 停止。
  void _startReplay(Map<String, dynamic> m) {
    final id = (m['id'] as num).toInt();
    if (_replayMsgId == id) {
      _stopReplay();
      return;
    }
    final strokes = (m['strokes'] as List).cast<Map<String, dynamic>>();
    if (strokes.isEmpty) return;
    final total = _BubbleTimeline(strokes).total /
        _bubbleReplaySpeed.clamp(1, 8).toDouble();
    setState(() => _replayMsgId = id);
    _replayCtrl.duration =
        Duration(milliseconds: (total.clamp(0.6, 20.0) * 1000).round());
    _replayCtrl.forward(from: 0);
  }

  void _stopReplay() {
    if (_replayMsgId == null) return;
    _replayCtrl.stop();
    setState(() => _replayMsgId = null);
  }

  Widget _buildBubbleTab() {
    final l = AppLocalizations.of(context)!;
    return _tabBackground(
        'assets/tab_bg_settings.jpg',
        Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 12, 4),
              child: Row(children: [
                Text(l.bubbleTab,
                    style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: _inkFaint)),
                const Spacer(),
                IconButton(
                  tooltip: l.refreshBubbles,
                  icon: _bubblesLoading
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child:
                              CircularProgressIndicator(strokeWidth: 2))
                      : const Icon(Icons.refresh, size: 20),
                  onPressed: _bubblesLoading ? null : _loadBubbles,
                ),
              ]),
            ),
            Expanded(
              child: (_bubbleMsgs.isEmpty && !_bubblesLoading)
                  ? Center(
                      child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Text(l.bubblesEmpty,
                            textAlign: TextAlign.center,
                            style: TextStyle(
                                fontSize: 13, color: Colors.grey.shade600)),
                      ),
                    )
                  : ListView.builder(
                      controller: _bubbleScroll,
                      padding: const EdgeInsets.fromLTRB(12, 4, 12, 16),
                      itemCount: _bubbleMsgs.length,
                      itemBuilder: (ctx, i) =>
                          _bubbleMessage(_bubbleMsgs[i]),
                    ),
            ),
          ],
        ));
  }

  /// 一条气泡: 右对齐(用户口吻), 内容 = 该段笔迹的矢量重绘 + 完成时刻。
  /// 点播放键重播(按书写时序逐笔重现); 重播中图标变停止。
  Widget _bubbleMessage(Map<String, dynamic> m) {
    final strokes =
        (m['strokes'] as List).cast<Map<String, dynamic>>();
    final tsMs = (m['ts_ms'] as num).toInt();
    final dt = DateTime.fromMillisecondsSinceEpoch(tsMs);
    final time = '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    final l = AppLocalizations.of(context)!;
    final replaying = _replayMsgId == (m['id'] as num).toInt();

    // 由笔迹包围盒推气泡内容尺寸(上限内等比, 不放大超过 1.5 倍)
    double minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9;
    for (final st in strokes) {
      final pts = (st['pts'] as List).cast<num>();
      for (var i = 0; i + 2 < pts.length; i += 3) {
        final x = pts[i].toDouble();
        final y = pts[i + 1].toDouble();
        minX = minX < x ? minX : x;
        maxX = maxX > x ? maxX : x;
        minY = minY < y ? minY : y;
        maxY = maxY > y ? maxY : y;
      }
    }
    const maxW = 300.0, maxH = 200.0;
    final bw = (maxX - minX).clamp(1.0, double.infinity).toDouble();
    final bh = (maxY - minY).clamp(1.0, double.infinity).toDouble();
    final k = ((maxW / bw) < (maxH / bh) ? maxW / bw : maxH / bh)
        .clamp(0.15, 1.5)
        .toDouble();
    final size = Size(bw * k + 4, bh * k + 4);

    // 重播中: AnimatedBuilder 逐帧喂进度; 静态 = 满帧(同一画法)
    Widget ink = replaying
        ? AnimatedBuilder(
            animation: _replayCtrl,
            builder: (_, _) => CustomPaint(
              size: size,
              painter: _BubbleStrokePainter(strokes, progress: _replayCtrl.value),
            ),
          )
        : CustomPaint(
            size: size,
            painter: _BubbleStrokePainter(strokes),
          );

    return Align(
      alignment: Alignment.centerRight,
      child: GestureDetector(
        onSecondaryTapUp: (d) => _showBubbleMenu(d.globalPosition, m),
        child: Container(
          margin: const EdgeInsets.fromLTRB(56, 6, 12, 6),
          padding: const EdgeInsets.fromLTRB(10, 8, 10, 6),
          decoration: BoxDecoration(
            color: const Color(0xFFD9F0FF), // 淡蓝 = 用户口吻(发送侧)
            borderRadius: const BorderRadius.only(
              topLeft: Radius.circular(12),
              topRight: Radius.circular(12),
              bottomLeft: Radius.circular(12),
              bottomRight: Radius.circular(2), // 右下小角 = 发送侧标识
            ),
            border: Border.all(color: Colors.grey.shade300),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            mainAxisSize: MainAxisSize.min,
            children: [
              ink,
              const SizedBox(height: 2),
              Row(mainAxisSize: MainAxisSize.min, children: [
                Text(time,
                    style:
                        TextStyle(fontSize: 10, color: Colors.grey.shade600)),
                SizedBox(
                  width: 22,
                  height: 22,
                  child: IconButton(
                    padding: EdgeInsets.zero,
                    iconSize: 15,
                    visualDensity: VisualDensity.compact,
                    tooltip: replaying ? l.stopReplay : l.replayBubble,
                    icon: Icon(replaying ? Icons.stop : Icons.play_arrow),
                    onPressed: () => _startReplay(m),
                  ),
                ),
              ]),
            ],
          ),
        ),
      ),
    );
  }

  /// 气泡右键菜单: 复制为 SVG 动画(粘贴进浏览器/文档即播放手写显现)。
  /// 菜单锚点照 _showDetailMenu 同款: 相对 Overlay 反算 + 夹紧内缘
  /// (RelativeRect 的 right/bottom 是到边缘的距离, 不是坐标)。
  void _showBubbleMenu(Offset globalPos, Map<String, dynamic> m) {
    final hasInk = (m['strokes'] as List?)?.isNotEmpty ?? false;
    if (!hasInk) return;
    final overlay =
        Overlay.of(context).context.findRenderObject() as RenderBox;
    final rel = overlay.globalToLocal(globalPos);
    const menuW = 200.0, menuH = 44.0;
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
        PopupMenuItem(
            value: 'svg', height: 36,
            child: Row(children: [
              Icon(Icons.copy, size: 16),
              SizedBox(width: 8),
              Text(AppLocalizations.of(context)!.bubbleCopySvg,
                  style: TextStyle(fontSize: 13)),
            ])),
      ],
    ).then((v) {
      if (v == 'svg') unawaited(_copyBubbleSvg(m));
    });
  }

  Future<void> _copyBubbleSvg(Map<String, dynamic> m) async {
    try {
      final strokes =
          (m['strokes'] as List).cast<Map<String, dynamic>>();
      final svg = _bubbleSvgAnimation(
          strokes, _BubbleTimeline.revealRate * _bubbleReplaySpeed);
      if (svg.isEmpty) return;
      await Clipboard.setData(ClipboardData(text: svg));
      if (!mounted) return;
      ScaffoldMessenger.maybeOf(context)?.showSnackBar(SnackBar(
        content: Text(AppLocalizations.of(context)!.bubbleSvgCopied),
        duration: const Duration(seconds: 2),
      ));
    } catch (e) {
      _toastError('$e');
    }
  }
}

/// 气泡内的笔迹重绘: ShareStroke 形状的点列 → 等比重绘。
/// 每段按该点自带的压感宽度逐段绘制(与覆盖层 cairo raw_draw_segment
/// 同款), round cap 让相邻段自然融合; 宽度随缩放 k, 下限 1 屏幕像素。
class _BubbleStrokePainter extends CustomPainter {
  final List<Map<String, dynamic>> strokes;
  /// 重播进度 0..1(默认 1 = 完整笔迹)。时间线见 _BubbleTimeline:
  /// 笔内按弧长匀速, 笔间按真实停顿(g 字段), 跨越中的末段线性插值。
  final double progress;
  _BubbleStrokePainter(this.strokes, {this.progress = 1.0});

  @override
  void paint(Canvas canvas, Size size) {
    double minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9;
    for (final st in strokes) {
      final pts = (st['pts'] as List).cast<num>();
      for (var i = 0; i + 2 < pts.length; i += 3) {
        final x = pts[i].toDouble();
        final y = pts[i + 1].toDouble();
        minX = minX < x ? minX : x;
        maxX = maxX > x ? maxX : x;
        minY = minY < y ? minY : y;
        maxY = maxY > y ? maxY : y;
      }
    }
    if (minX > maxX || minY > maxY) return;
    const pad = 2.0;
    final bw = (maxX - minX).clamp(1.0, double.infinity).toDouble();
    final bh = (maxY - minY).clamp(1.0, double.infinity).toDouble();
    final k = (((size.width - 2 * pad) / bw) < ((size.height - 2 * pad) / bh)
            ? (size.width - 2 * pad) / bw
            : (size.height - 2 * pad) / bh)
        .clamp(0.01, 10.0)
        .toDouble();
    canvas.translate(pad - minX * k, pad - minY * k);
    canvas.scale(k, k);

    final minW = 1.0 / k; // 下限 = 缩放后仍至少 1 屏幕像素
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final tl = _BubbleTimeline(strokes);
    final elapsed = progress.clamp(0.0, 1.0) * tl.total;
    for (var si = 0; si < strokes.length; si++) {
      if (elapsed <= tl.starts[si]) break; // 时间线单调, 后面的笔还没开始
      final st = strokes[si];
      final pts = (st['pts'] as List).cast<num>();
      if (pts.length < 3) continue;
      paint.color = Color(0xFF000000 | (st['rgb'] as num).toInt() & 0xFFFFFF);
      final x0 = pts[0].toDouble(), y0 = pts[1].toDouble();
      final w0 = pts[2].toDouble();
      if (pts.length == 3) {
        // 单点笔画: 出现即整点(实心圆点, 与 cairo 起笔圆点一致)
        paint
          ..style = PaintingStyle.fill
          ..strokeWidth = 0;
        canvas.drawCircle(Offset(x0, y0), (w0 < minW ? minW : w0) * 0.5, paint);
        paint.style = PaintingStyle.stroke;
        continue;
      }
      // 该笔已揭示的弧长(笔内匀速 = 揭示秒数 × 基准速率)
      final revealLen =
          ((elapsed - tl.starts[si]) * _BubbleTimeline.revealRate)
              .clamp(0.0, tl.arcs[si])
              .toDouble();
      var drawn = 0.0;
      var px = x0, py = y0, pw = w0;
      for (var i = 3; i + 2 < pts.length; i += 3) {
        final x = pts[i].toDouble();
        final y = pts[i + 1].toDouble();
        final w = pts[i + 2].toDouble(); // 该点压感宽度(绝对线宽)
        final segLen =
            math.sqrt((x - px) * (x - px) + (y - py) * (y - py));
        if (drawn + segLen <= revealLen) {
          // 整段都在进度内
          paint.strokeWidth = w < minW ? minW : w;
          canvas.drawLine(Offset(px, py), Offset(x, y), paint);
          drawn += segLen;
          px = x;
          py = y;
          pw = w;
          if (drawn >= revealLen) break;
        } else {
          // 进度停在本段中间: 画到插值点为止(端点与宽度同插值)
          final t = segLen > 0 ? (revealLen - drawn) / segLen : 1.0;
          final ex = px + (x - px) * t;
          final ey = py + (y - py) * t;
          final ew = pw + (w - pw) * t;
          paint.strokeWidth = ew < minW ? minW : ew;
          canvas.drawLine(Offset(px, py), Offset(ex, ey), paint);
          break;
        }
      }
    }
  }

  @override
  bool shouldRepaint(covariant _BubbleStrokePainter old) => true;
}

/// 重播时间线: 第 i 笔从 starts[i] 秒开始, 画 arcs[i]/rate 秒; 笔内按弧长
/// 匀速, 笔间停顿用真实 gap(每段钳 2s, 防长思考拖垮重播)。静态绘制 =
/// progress 1.0, 与时间线无耦合。rate 可调(重播 = 默认 1200, SVG 动画 =
/// 1200 × 速度倍率)。
class _BubbleTimeline {
  static const revealRate = 1200.0; // px/s
  final List<double> starts = [];
  final List<double> arcs = [];
  double total = 0;

  _BubbleTimeline(List<Map<String, dynamic>> strokes,
      {double rate = revealRate}) {
    for (final st in strokes) {
      final gapMs = ((st['g'] as num?)?.toDouble() ?? 0.0).clamp(0.0, 2000.0);
      total += gapMs / 1000.0;
      starts.add(total);
      final pts = (st['pts'] as List).cast<num>();
      var arc = 0.0;
      for (var i = 3; i + 2 < pts.length; i += 3) {
        final dx = pts[i].toDouble() - pts[i - 3].toDouble();
        final dy = pts[i + 1].toDouble() - pts[i - 2].toDouble();
        arc += math.sqrt(dx * dx + dy * dy);
      }
      arcs.add(arc);
      total += arc / rate;
    }
  }
}

/// SVG 数值格式化(2 位小数足够: 坐标已按 0.1px 取整, 宽度按 0.25 分桶)。
String _svgN(num v) => v.toStringAsFixed(2);

/// SVG SMIL 时间格式化(秒, 3 位小数)。
String _svgT(double s) => '${s.toStringAsFixed(3)}s';

/// 把一条消息的笔迹烘焙成 SMIL「手写显现」动画 SVG(自包含文本, 粘进
/// 浏览器/支持 SVG 的文档即播放): 每笔按宽度 0.25px 分桶切成 run
/// (run 内宽度一致 = 一个 path), 用虚线法逐段显现 —— 初始
/// dasharray/dashoffset = run 弧长(不可见), animate 到 0(fill freeze)。
/// 时间线与重播同源(_BubbleTimeline): 笔间真实停顿(g, 钳 2s)+
/// 笔内按弧长匀速(rate px/s, 复制时 = 1200 × 面板速度倍率)。
/// 取舍: 忽略 SMIL 的静态渲染器显示首帧 = 空白(观看书写过程的第一帧
/// 天然为空); 单点笔画用实心圆点淡入。
String _bubbleSvgAnimation(List<Map<String, dynamic>> strokes, double rate) {
  if (strokes.isEmpty) return '';

  // 包围盒(与 _BubbleStrokePainter 同源, pad 2)
  double minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9;
  for (final st in strokes) {
    final pts = (st['pts'] as List).cast<num>();
    for (var i = 0; i + 2 < pts.length; i += 3) {
      final x = pts[i].toDouble();
      final y = pts[i + 1].toDouble();
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
  }
  if (minX > maxX || minY > maxY) return '';
  const pad = 2.0;
  final vx = minX - pad, vy = minY - pad;
  final vw = maxX - minX + 2 * pad, vh = maxY - minY + 2 * pad;

  final tl = _BubbleTimeline(strokes, rate: rate);
  final buf = StringBuffer();
  buf.write('<svg xmlns="http://www.w3.org/2000/svg" '
      'viewBox="${_svgN(vx)} ${_svgN(vy)} ${_svgN(vw)} ${_svgN(vh)}" '
      'width="${_svgN(vw)}" height="${_svgN(vh)}">\n');

  for (var si = 0; si < strokes.length; si++) {
    final st = strokes[si];
    final pts = (st['pts'] as List).cast<num>();
    if (pts.length < 3) continue;
    final rgb =
        ((st['rgb'] as num).toInt() & 0xFFFFFF).toRadixString(16).padLeft(6, '0');
    final strokeBegin = tl.starts[si];
    final strokeArc = tl.arcs[si];
    final strokeDur = (strokeArc / rate).clamp(0.01, double.infinity).toDouble();
    int nP(int p) => 3 * p; // 点 p 的 pts 下标基址
    double px(int p) => pts[nP(p)].toDouble();
    double py(int p) => pts[nP(p) + 1].toDouble();
    double pw(int p) => pts[nP(p) + 2].toDouble();
    double qx(double w) => (w * 4).round() / 4; // 0.25px 宽度分桶

    if (pts.length == 3) {
      // 单点笔画: 实心圆点半径 = 半宽, 淡入
      final r = (pw(0) * 0.5).clamp(0.3, double.infinity);
      buf.write('<circle cx="${_svgN(px(0))}" cy="${_svgN(py(0))}" '
          'r="${_svgN(r)}" fill="#$rgb" opacity="0">'
          '<animate attributeName="opacity" from="0" to="1" '
          'begin="${_svgT(strokeBegin)}" dur="0.01s" fill="freeze"/>'
          '</circle>\n');
      continue;
    }
    if (strokeArc <= 0) {
      // 退化(点全部重叠): 整笔立即淡入
      final d = StringBuffer('M${_svgN(px(0))} ${_svgN(py(0))}');
      for (var p = 1; 3 * p + 2 < pts.length; p++) {
        d.write(' L${_svgN(px(p))} ${_svgN(py(p))}');
      }
      buf.write('<path d="$d" fill="none" stroke="#$rgb" '
          'stroke-width="${_svgN(pw(0))}" stroke-linecap="round" '
          'stroke-linejoin="round" opacity="0">'
          '<animate attributeName="opacity" from="0" to="1" '
          'begin="${_svgT(strokeBegin)}" dur="0.01s" fill="freeze"/></path>\n');
      continue;
    }

    // 段序列(段 k = 点 k→k+1, 宽度取到达点 w[k+1], 与 painter 同约定)
    final nSeg = pts.length ~/ 3 - 1;
    final cum = List<double>.filled(nSeg + 1, 0); // 段起点的累计弧长
    for (var k = 0; k < nSeg; k++) {
      final dx = px(k + 1) - px(k), dy = py(k + 1) - py(k);
      cum[k + 1] = cum[k] + math.sqrt(dx * dx + dy * dy);
    }
    // 相邻同宽度段合并为一个 run(一个 path)
    var a = 0;
    while (a < nSeg) {
      var b = a;
      while (b + 1 < nSeg && qx(pw(b + 2)) == qx(pw(a + 1))) {
        b++;
      }
      final runArc = cum[b + 1] - cum[a];
      final begin = strokeBegin + strokeDur * (cum[a] / strokeArc);
      final dur = (strokeDur * (runArc / strokeArc)).clamp(0.01, double.infinity);
      final dash = runArc + 0.01; // +ε: 零长 run 也能画出一个圆帽点
      final d = StringBuffer('M${_svgN(px(a))} ${_svgN(py(a))}');
      for (var p = a + 1; p <= b + 1; p++) {
        d.write(' L${_svgN(px(p))} ${_svgN(py(p))}');
      }
      buf.write('<path d="$d" fill="none" stroke="#$rgb" '
          'stroke-width="${_svgN(qx(pw(a + 1)))}" stroke-linecap="round" '
          'stroke-linejoin="round" stroke-dasharray="${_svgN(dash)}" '
          'stroke-dashoffset="${_svgN(dash)}">'
          '<animate attributeName="stroke-dashoffset" '
          'from="${_svgN(dash)}" to="0" begin="${_svgT(begin)}" '
          'dur="${_svgT(dur)}" fill="freeze"/></path>\n');
      a = b + 1;
    }
  }
  buf.write('</svg>\n');
  return buf.toString();
}
