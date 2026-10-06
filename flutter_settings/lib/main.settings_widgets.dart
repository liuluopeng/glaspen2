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
