part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 检查更新与关于 ──

String _fmtMb(int bytes) =>
    '${(bytes / 1048576).toStringAsFixed(1)} MB';

extension _UpdateUi on _SettingsPageState {
  /// 取当前版本号(桥不可用时静默失败, 版本栏保持 —)。
  Future<void> _loadAppVersion() async {
    try {
      final v = await _bridge.appVersion();
      if (mounted && v.isNotEmpty) setState(() => _appVersion = v);
    } catch (e) {
      debugPrint('[About] appVersion failed: $e');
    }
  }

  /// 手动「检查更新」:一次 GitHub API 往返, 成功/失败都放进 _updateResult。
  Future<void> _checkUpdate() async {
    if (_checkingUpdate) return;
    setState(() {
      _checkingUpdate = true;
      _updateResult = null;
    });
    Map<dynamic, dynamic> result;
    try {
      result = await _bridge.checkUpdate();
      // 空 map = 管道没连上(Windows),或对端没实现该消息
      if (result.isEmpty) {
        result = {'ok': false, 'error': '未连接到主程序'};
      }
    } catch (e) {
      result = {'ok': false, 'error': '$e'};
    }
    if (!mounted) return;
    setState(() {
      _checkingUpdate = false;
      _updateResult = result;
    });
  }

  /// 「关于」区:当前版本 + 检查更新(结果:已是最新 / 新版本+下载页 / 失败原因)。
  /// 「立即更新」第一步:确认对话框(版本 + release notes + 提示会重启)。
  Future<bool> _confirmUpdate() async {
    final r = _updateResult!;
    final tag = (r['latest'] as String?) ?? '';
    final notes = (r['notes'] as String?) ?? '';
    final res = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('更新到 $tag ?'),
        content: SizedBox(
          width: 440,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                const Text(
                  '将下载新版安装包、退出并自动重启玻璃涂鸦。\n笔迹已实时存盘,不会丢失。',
                  style: TextStyle(fontSize: 13),
                ),
                if (notes.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  const Text('更新内容:',
                      style: TextStyle(
                          fontSize: 13, fontWeight: FontWeight.bold)),
                  const SizedBox(height: 6),
                  Text(notes,
                      style: const TextStyle(
                          fontSize: 12.5, color: _inkFaint, height: 1.5)),
                ],
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('取消')),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: const Text('下载并更新')),
        ],
      ),
    );
    return res ?? false;
  }

  /// 「立即更新」:确认 → 下载(进度)→ 解包 → 等待用户点「立即重启更新」。
  Future<void> _startUpdate() async {
    if (_upd != _UpdPhase.idle || _updateResult == null) return;
    if (!await _confirmUpdate() || !mounted) return;
    final r = _updateResult!;
    setState(() {
      _upd = _UpdPhase.downloading;
      _updTag = (r['latest'] as String?) ?? '';
      _updReceived = 0;
      _updTotal = 0;
      _updError = '';
    });
    _updSub = _bridge.downloadUpdate().listen(
      _onUpdateFrame,
      onError: (Object e) {
        if (!mounted) return;
        setState(() {
          _upd = _UpdPhase.failed;
          _updError = '下载失败:$e';
        });
      },
      onDone: () => _updSub = null,
    );
  }

  /// 下载流的每一帧;`done` 帧决定去解包还是报错。
  void _onUpdateFrame(Map<dynamic, dynamic> f) {
    if (!mounted) return;
    if (f['done'] != true) {
      setState(() {
        _updReceived = (f['received'] as num?)?.toInt() ?? 0;
        _updTotal = (f['total'] as num?)?.toInt() ?? 0;
      });
      return;
    }
    _updSub?.cancel();
    _updSub = null;
    final err = (f['error'] as String?) ?? '';
    if (err.isNotEmpty) {
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = err;
      });
      return;
    }
    _stageUpdate();
  }

  /// 下载完成 → 解包 DMG(dmg 路径 Rust 侧自己找, 不经 UI 传参)。
  Future<void> _stageUpdate() async {
    setState(() => _upd = _UpdPhase.staging);
    try {
      final s = await _bridge.stageUpdate(_updTag);
      if (!mounted) return;
      setState(() {
        if (s['ok'] == true) {
          _upd = _UpdPhase.ready;
        } else {
          _upd = _UpdPhase.failed;
          _updError = (s['message'] as String?) ?? '解包失败';
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = '解包失败:$e';
      });
    }
  }

  /// 「立即重启更新」:拉起帮手 → 本进程退出(正常情况下 Future 永不返回)。
  Future<void> _applyUpdate() async {
    setState(() => _upd = _UpdPhase.applying);
    try {
      final a = await _bridge.applyUpdate();
      // 走到这里 = 退出失败(成功的路径进程已经没了)
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = (a['message'] as String?) ?? '退出更新失败';
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _upd = _UpdPhase.failed;
        _updError = '退出更新失败:$e';
      });
    }
  }

  /// 取消下载(解包后的 ready 状态不给取消 —— 文件已就绪,重启才生效)。
  void _cancelUpdate() {
    _updSub?.cancel();
    _updSub = null;
    setState(() => _upd = _UpdPhase.idle);
  }

  /// 「立即更新」进行中的 UI(下载进度 / 解包中 / 待重启 / 失败)。
  Widget _buildUpdateProgress(TextStyle faint) {
    switch (_upd) {
      case _UpdPhase.downloading:
        final value = _updTotal > 0 ? _updReceived / _updTotal : null;
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          LinearProgressIndicator(value: value),
          const SizedBox(height: 8),
          Row(children: [
            Expanded(
              child: Text(
                _updTotal > 0
                    ? '下载中 ${_fmtMb(_updReceived)} / ${_fmtMb(_updTotal)}'
                    : '下载中…',
                style: faint,
              ),
            ),
            TextButton(
                onPressed: _cancelUpdate,
                child: const Text('取消',
                    style: TextStyle(fontSize: 13))),
          ]),
        ]);
      case _UpdPhase.staging:
        return Row(children: [
          const SizedBox(
              width: 14, height: 14,
              child: CircularProgressIndicator(strokeWidth: 2)),
          const SizedBox(width: 8),
          Text('下载完成,正在解包校验…', style: faint),
        ]);
      case _UpdPhase.ready:
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Text('已下载并解包完成。重启后生效。',
              style: TextStyle(fontSize: 13, color: _ink)),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: _applyUpdate,
            child: const Text('立即重启更新'),
          ),
        ]);
      case _UpdPhase.applying:
        return Row(children: [
          const SizedBox(
              width: 14, height: 14,
              child: CircularProgressIndicator(strokeWidth: 2)),
          const SizedBox(width: 8),
          Text('正在退出并重启…', style: faint),
        ]);
      case _UpdPhase.failed:
        return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('更新失败:$_updError',
              style: const TextStyle(fontSize: 13, color: _penRed)),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: () => setState(() => _upd = _UpdPhase.idle),
            child: const Text('重试'),
          ),
        ]);
      case _UpdPhase.idle:
        return const SizedBox.shrink();
    }
  }

  Widget _buildAboutSection() {
    const faint = TextStyle(fontSize: 13, color: _inkFaint);

    final Widget status;
    if (_upd != _UpdPhase.idle) {
      // 点过「立即更新」之后,状态区整体交给更新状态机
      status = _buildUpdateProgress(faint);
    } else if (_checkingUpdate) {
      status = Row(children: [
        const SizedBox(
            width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2)),
        const SizedBox(width: 8),
        const Text('正在检查…', style: faint),
      ]);
    } else if (_updateResult != null) {
      final r = _updateResult!;
      if (r['ok'] == true && r['hasUpdate'] == true) {
        final url = (r['url'] as String?) ?? '';
        status = Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('发现新版本 ${r['latest']}',
              style: const TextStyle(
                  fontSize: 14, fontWeight: FontWeight.bold, color: _penRed)),
          const SizedBox(height: 8),
          Wrap(spacing: 8, runSpacing: 6, children: [
            OutlinedButton(
              onPressed: _startUpdate,
              child: const Text('立即更新'),
            ),
            OutlinedButton.icon(
              icon: const Icon(Icons.open_in_new, size: 15),
              label: const Text('打开下载页'),
              onPressed: () async {
                try {
                  await _bridge.openUrl(url.isEmpty ? _releasesPage : url);
                } catch (e) {
                  debugPrint('[About] openUrl failed: $e');
                }
              },
            ),
          ]),
        ]);
      } else if (r['ok'] == true) {
        status = Text('已是最新版本 (v${r['current']})', style: faint);
      } else {
        status = Text('检查失败:${r['error'] ?? '未知错误'}',
            style: const TextStyle(fontSize: 13, color: _penRed));
      }
    } else {
      status = const Text('点一下向 GitHub 查询最新正式版(需联网)',
          style: TextStyle(fontSize: 12, color: _inkFaint));
    }

    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Row(children: [
        const Text('当前版本', style: TextStyle(fontSize: 14, color: _ink)),
        const Spacer(),
        Text(_appVersion == null ? '—' : 'v$_appVersion',
            style: faint.copyWith(fontSize: 14)),
      ]),
      const SizedBox(height: 10),
      OutlinedButton(
        onPressed: _checkingUpdate ? null : _checkUpdate,
        child: const Text('检查更新'),
      ),
      const SizedBox(height: 10),
      status,
    ]);
  }
}
