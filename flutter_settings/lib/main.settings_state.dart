part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 设置同步与聊天集成状态 ──

extension _SettingsStateUi on _SettingsPageState {
  /// 覆盖层推送的设置更新: **只覆盖帧里出现的字段**, 其余保持当前值。
  /// 覆盖层(Windows 管道 / macOS FRB)可能只推局部字段(如只推 shortcuts /
  /// infiniteCanvas), 若对缺失字段用 `_b(null)=false` 兜底, 一次推送就会把
  /// 面板其它开关全部清成关、tab 数也算错。
  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    _settingsLoadGen++; // 服务端推送视为最新状态, 作废在途的旧加载
    if (!mounted) return;
    final has = s.containsKey;
    bool b(String key, bool cur) => has(key) ? _b(s[key]) : cur;
    int i(String key, int cur) => (s[key] as num?)?.toInt() ?? cur;
    double d(String key, double cur) => (s[key] as num?)?.toDouble() ?? cur;
    final wasInfinite = _infiniteCanvas;
    final wasFreeTab = _showFreeCanvas;
    setState(() {
      _selectedColor = i('color', _selectedColor);
      _selectedWidth = i('width', _selectedWidth);
      _pressureMonitor = b('pressureMonitor', _pressureMonitor);
      _launchAtLogin = b('launchAtLogin', _launchAtLogin);
      _showGrid = b('grid', _showGrid);
      _gridFollowStrokes = b('gridFollowStrokes', _gridFollowStrokes);
      _glassFollowStrokes = _bOr(s['glassFollowStrokes'], _glassFollowStrokes);
      _softShadow = b('softShadow', _softShadow);
      _invertInk = b('invertInk', _invertInk);
      _invertFps = i('invertFps', _invertFps);
      _outlineEnabled = b('outline', _outlineEnabled);
      _infiniteCanvas = b('infiniteCanvas', _infiniteCanvas);
      _minimapEnabled = b('minimap', _minimapEnabled);
      _gridSizeValue = i('gridSize', _gridSizeValue);
      _gridDividerValue = i('gridDivider', _gridDividerValue);
      _gifFps = i('gifFps', _gifFps);
      _gifResolution = d('gifResolution', _gifResolution);
      _gifSpeed = d('gifSpeed', _gifSpeed);
      _gifEndMode = i('gifEndMode', _gifEndMode);
      _flipEffect = i('flipEffect', _flipEffect);
      _showFreeCanvas = b('showFreeCanvas', _showFreeCanvas);
      _showBubbleTab = b('showBubbleTab', _showBubbleTab);
      _bubbleReplaySpeed = i('bubbleReplaySpeed', _bubbleReplaySpeed).clamp(1, 8);
      _bubbleReplayHoldLast = _bOr(s['bubbleReplayHoldLast'], _bubbleReplayHoldLast);
      _bubbleReplayAuto = b('bubbleReplayAuto', _bubbleReplayAuto);
      _bubbleAutoNewPage = b('bubbleAutoNewPage', _bubbleAutoNewPage);
      _shareCanvas = b('shareCanvas', _shareCanvas);
      _notebookGlass = _bOr(s['notebookStyle'], _notebookGlass);
      _panelTransparent = b('panelTransparent', _panelTransparent);
      if (has('language')) {
        _language = (s['language'] as String?) == 'en' ? 'en' : 'zh';
      }
      if (has('shortcuts')) {
        _shortcuts = _decodeShortcuts(s['shortcuts']);
      }
      _syncTabCount();
    });
    // 画布模式被外部改(快捷键切自由涂鸦/覆盖层推设置)时, TabBar 选中项必须
    // 跟着走 —— 否则"玻璃在自由涂鸦、面板停在活页本", 用户再点活页本就会
    // 把自由涂鸦关掉(用户报的严重错位)。
    if (wasInfinite != _infiniteCanvas || wasFreeTab != _showFreeCanvas) {
      _syncTabToCanvasMode(force: true);
    }
    _applyLanguage(_language);
  }

  Future<void> _loadSettings() async {
    final gen = ++_settingsLoadGen;
    try {
      final settings = await _bridge.getSettings();
      // 等待期间用户改过设置 / 收到推送 → 丢弃这份过期快照
      if (gen != _settingsLoadGen) return;
      if (mounted && settings.isNotEmpty) {
        final wasInfinite = _infiniteCanvas;
        final wasFreeTab = _showFreeCanvas;
        setState(() {
          _selectedColor = (settings['color'] as num?)?.toInt() ?? 0;
          _selectedWidth = (settings['width'] as num?)?.toInt() ?? 2;
          _pressureMonitor = _b(settings['pressureMonitor']);
          _showGrid = _b(settings['grid']);
          _gridFollowStrokes = _b(settings['gridFollowStrokes']);
          _glassFollowStrokes = _bOr(settings['glassFollowStrokes'], _glassFollowStrokes);
          _softShadow = _b(settings['softShadow']);
          _invertInk = _b(settings['invertInk']);
          _invertFps = (settings['invertFps'] as num?)?.toInt() ?? _invertFps;
          _infiniteCanvas = _b(settings['infiniteCanvas']);
          _minimapEnabled = _b(settings['minimap']);
          _gridSizeValue = (settings['gridSize'] as num?)?.toInt() ?? _gridSizeValue;
          _gridDividerValue = (settings['gridDivider'] as num?)?.toInt() ?? _gridDividerValue;
          _gifFps = (settings['gifFps'] as num?)?.toInt() ?? 15;
          _gifResolution = (settings['gifResolution'] as num?)?.toDouble() ?? 0.5;
          _gifSpeed = (settings['gifSpeed'] as num?)?.toDouble() ?? 2.0;
          _gifEndMode = (settings['gifEndMode'] as num?)?.toInt() ?? 1;
          _flipEffect = (settings['flipEffect'] as num?)?.toInt() ?? _flipEffect;
          _nasUrl.text = (settings['nasUrl'] as String?) ?? '';
          _chatApiBase.text = (settings['chatApiBase'] as String?) ?? '';
          _chatUser.text = (settings['chatUser'] as String?) ?? '';
          _chatHasPassword = _b(settings['chatHasPassword']);
          _chatIntegration = _b(settings['chatIntegration']);
          _showFreeCanvas = _b(settings['showFreeCanvas']);
          _showBubbleTab = _b(settings['showBubbleTab']);
          _bubbleReplaySpeed =
              ((settings['bubbleReplaySpeed'] as num?)?.toInt() ?? 1)
                  .clamp(1, 8);
          _bubbleReplayHoldLast = _bOr(settings['bubbleReplayHoldLast'], true);
          _bubbleReplayAuto = _b(settings['bubbleReplayAuto']);
          _bubbleAutoNewPage = _b(settings['bubbleAutoNewPage']);
          _shareCanvas = _b(settings['shareCanvas']);
          _notebookGlass = _b(settings['notebookStyle']);
          _panelTransparent = _b(settings['panelTransparent']);
          _language = (settings['language'] as String?) == 'en' ? 'en' : 'zh';
          _shortcuts = _decodeShortcuts(settings['shortcuts']);
          _syncTabCount();
        });
        // 首次拿到设置: 直接把面板停在当前画布模式对应的 tab(自由涂鸦/活页本),
        // 让"面板选的模式"和"玻璃实际模式"一致; 不是首次则只在模式变化时跟随。
        if (!_initialTabApplied) {
          _initialTabApplied = true;
          _syncTabToCanvasMode(force: true);
        } else if (wasInfinite != _infiniteCanvas || wasFreeTab != _showFreeCanvas) {
          _syncTabToCanvasMode(force: true);
        }
        _applyLanguage(_language);
        // 设置就绪后顺手预取活页本首屏缩略图(一次批量往返),
        // 等用户切到活页本时直接就有图,连加载态都不用出现。
        if (_pages.isEmpty && !_pagesLoading) unawaited(_loadPages());
      } else if (mounted) {
        // Windows 管道未就绪时 getSettings 返回空:稍后重试
        _reloadTimer = Timer(const Duration(seconds: 2), _loadSettings);
      }
    } catch (e) {
      // 桥不可用时退回默认值;FRB 初始化失败也会走这里,打日志便于定位
      // (Rust 符号没导出、库没链接等都会在首次调用时暴露出来)
      debugPrint('[Settings] load failed: $e');
    }
  }

  void _setSetting(String key, dynamic value) {
    // 用户刚做的选择优先于任何在途的首帧加载
    _settingsLoadGen++;
    // 这是"发出去就不管"的写入:失败只记日志,不该因为桥异常打断 UI
    _bridge.setSetting(key, value).catchError((Object e) {
      debugPrint('[Settings] set "$key" failed: $e');
    });
  }

  // ── 涂鸦身份(手写消息登录 axum)──

  /// 保存三个字段;密码为空 = 保持已存的(面板不回显密码)。
  Future<void> _saveChatAuth() async {
    await _bridge.setSetting('chatApiBase', _chatApiBase.text.trim());
    await _bridge.setSetting('chatUser', _chatUser.text.trim());
    if (_chatPassword.text.isNotEmpty) {
      await _bridge.setSetting('chatPassword', _chatPassword.text);
    }
  }

  /// 「测试登录」:先保存,再强制登录一次,回显结果。
  Future<void> _testChatLogin() async {
    setState(() => _chatLoginBusy = true);
    try {
      await _saveChatAuth();
      final err = await _bridge.testChatLogin();
      if (!mounted) return;
      setState(() {
        _chatHasPassword = _chatHasPassword || _chatPassword.text.isNotEmpty;
        _chatLoginMsg =
            err.isEmpty ? AppLocalizations.of(context)!.signedInHandwrittenMessages : AppLocalizations.of(context)!.signInFailedP1(err);
      });
    } catch (e) {
      if (mounted) setState(() => _chatLoginMsg = AppLocalizations.of(context)!.signInFailedP1(e));
    } finally {
      if (mounted) setState(() => _chatLoginBusy = false);
    }
  }

  Widget _buildChatAuthSection() {
    const faint12 = TextStyle(fontSize: 12, color: _inkFaint);
    // 总开关:关 = 只剩开关本身;⌘⌃2/⌘⌃3 直通,不显登录与共享。
    final master = _tile(SwitchListTile(
      title: Text(AppLocalizations.of(context)!.enableHandwrittenMessagesSharing,
          style: TextStyle(fontSize: 14)),
      subtitle: Text(AppLocalizations.of(context)!.whenOff23,
          style: TextStyle(fontSize: 12)),
      value: _chatIntegration,
      dense: true,
      contentPadding: EdgeInsets.zero,
      onChanged: (v) {
        setState(() => _chatIntegration = v);
        _setSetting('chatIntegration', v);
      },
    ));
    if (!_chatIntegration) {
      return Column(crossAxisAlignment: CrossAxisAlignment.start,
          children: [master]);
    }

    const faint = TextStyle(fontSize: 13, color: _inkFaint);
    InputDecoration deco(String label, String hint) => InputDecoration(
          labelText: label,
          hintText: hint,
          isDense: true,
          border: const OutlineInputBorder(),
        );
    final Widget status;
    if (_chatLoginBusy) {
      status = Row(children: [
        SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2)),
        SizedBox(width: 8),
        Text(AppLocalizations.of(context)!.signingIn, style: faint),
      ]);
    } else if (_chatLoginMsg.isNotEmpty) {
      final ok = _chatLoginMsg.startsWith(AppLocalizations.of(context)!.signedIn);
      status = Text(_chatLoginMsg,
          style: TextStyle(
              fontSize: 12.5, color: ok ? const Color(0xFF00B16E) : _penRed));
    } else if (_chatHasPassword) {
      status = Text(AppLocalizations.of(context)!.accountConfigured23,
          style: faint);
    } else {
      status = Text(AppLocalizations.of(context)!.notConfiguredMessagingStill,
          style: faint);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        master,
        const SizedBox(height: 4),
        TextField(
          controller: _chatApiBase,
          decoration: deco(AppLocalizations.of(context)!.serverAddress, AppLocalizations.of(context)!.eGHttps192),
          style: const TextStyle(fontSize: 13),
          autocorrect: false,
          keyboardType: TextInputType.url,
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _chatUser,
          decoration: deco(AppLocalizations.of(context)!.username, ''),
          style: const TextStyle(fontSize: 13),
          autocorrect: false,
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _chatPassword,
          obscureText: true,
          decoration: deco(AppLocalizations.of(context)!.password, _chatHasPassword ? AppLocalizations.of(context)!.savedLeaveBlankTo : ''),
          style: const TextStyle(fontSize: 13),
          onSubmitted: (_) => _testChatLogin(),
        ),
        const SizedBox(height: 10),
        Row(children: [
          OutlinedButton(
            onPressed: _chatLoginBusy ? null : _testChatLogin,
            child: Text(AppLocalizations.of(context)!.saveTestSignIn, style: TextStyle(fontSize: 13)),
          ),
          const SizedBox(width: 12),
          Expanded(child: status),
        ]),
        const SizedBox(height: 6),
        Text(
          AppLocalizations.of(context)!.authorAttributionFor2,
          style: faint12,
        ),
      ],
    );
  }
}
