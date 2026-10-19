part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 设置同步与聊天集成状态 ──

extension _SettingsStateUi on _SettingsPageState {
  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    _settingsLoadGen++; // 服务端推送视为最新状态, 作废在途的旧加载
    if (mounted) {
      setState(() {
        _selectedColor = (s['color'] as num?)?.toInt() ?? _selectedColor;
        _selectedWidth = (s['width'] as num?)?.toInt() ?? _selectedWidth;
        _pressureMonitor = _b(s['pressureMonitor']);
        _launchAtLogin = _b(s['launchAtLogin']);
        _showGrid = _b(s['grid']);
        _gridFollowStrokes = _b(s['gridFollowStrokes']);
        _glassFollowStrokes = s['glassFollowStrokes'] as bool? ?? _glassFollowStrokes;
        _softShadow = _b(s['softShadow']);
        _invertInk = _b(s['invertInk']);
        _invertFps = (s['invertFps'] as num?)?.toInt() ?? _invertFps;
        _outlineEnabled = _b(s['outline']);
        _infiniteCanvas = _b(s['infiniteCanvas']);
        _minimapEnabled = _b(s['minimap']);
        _gridSizeValue = (s['gridSize'] as num?)?.toInt() ?? _gridSizeValue;
        _gridDividerValue = (s['gridDivider'] as num?)?.toInt() ?? _gridDividerValue;
        _gifFps = (s['gifFps'] as num?)?.toInt() ?? _gifFps;
        _gifResolution = (s['gifResolution'] as num?)?.toDouble() ?? _gifResolution;
        _gifSpeed = (s['gifSpeed'] as num?)?.toDouble() ?? _gifSpeed;
        _gifEndMode = (s['gifEndMode'] as num?)?.toInt() ?? _gifEndMode;
        _flipEffect = (s['flipEffect'] as num?)?.toInt() ?? _flipEffect;
        _showFreeCanvas = _b(s['showFreeCanvas']);
        _shareCanvas = _b(s['shareCanvas']);
        _notebookGlass = s['notebookStyle'] as bool? ?? _notebookGlass;
        _panelTransparent = _b(s['panelTransparent']);
        _language = (s['language'] as String?) == 'en' ? 'en' : 'zh';
        _syncTabCount();
      });
      _applyLanguage(_language);
    }
  }

  Future<void> _loadSettings() async {
    final gen = ++_settingsLoadGen;
    try {
      final settings = await _bridge.getSettings();
      // 等待期间用户改过设置 / 收到推送 → 丢弃这份过期快照
      if (gen != _settingsLoadGen) return;
      if (mounted && settings.isNotEmpty) {
        setState(() {
          _selectedColor = (settings['color'] as num?)?.toInt() ?? 0;
          _selectedWidth = (settings['width'] as num?)?.toInt() ?? 2;
          _pressureMonitor = _b(settings['pressureMonitor']);
          _showGrid = _b(settings['grid']);
          _gridFollowStrokes = _b(settings['gridFollowStrokes']);
          _glassFollowStrokes =
              settings['glassFollowStrokes'] as bool? ?? _glassFollowStrokes;
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
          _chatApiBase.text = (settings['chatApiBase'] as String?) ?? '';
          _chatUser.text = (settings['chatUser'] as String?) ?? '';
          _chatHasPassword = _b(settings['chatHasPassword']);
          _chatIntegration = _b(settings['chatIntegration']);
          _showFreeCanvas = _b(settings['showFreeCanvas']);
          _shareCanvas = _b(settings['shareCanvas']);
          _notebookGlass = _b(settings['notebookStyle']);
          _panelTransparent = _b(settings['panelTransparent']);
          _language = (settings['language'] as String?) == 'en' ? 'en' : 'zh';
          _syncTabCount();
          _connected = true;
        });
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
