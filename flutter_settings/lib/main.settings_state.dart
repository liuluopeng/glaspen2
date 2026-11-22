part of 'main.dart';

// 本文件是 _SettingsPageState 的同库拆分: extension 里调 setState 属预期用法,
// State.@protected 的可见性按类外判定, 对同库 part 是误报。
// ignore_for_file: invalid_use_of_protected_member

// ── 设置同步与聊天集成状态(D11b: 通用快照合并)──
//
// 旧实现是逐键字段 + 逐键 setState(38 条登记项各写两遍: 首帧加载 + 覆盖层
// 推送)。D11b 起设置值只有一份 Map(_settingValues), 键/类型/平台可用性全部
// 来自后端 uiConfig 的登记表契约, 所以加载/推送是同一段通用代码。

/// 文本类设置键: 用户可能正在输入, 覆盖层局部推送不能回写输入框(旧行为)。
const _textSettingKeys = <String>{'chatApiBase', 'chatUser', 'nasUrl'};

extension _SettingsStateUi on _SettingsPageState {
  /// 覆盖层推送的设置更新: **只覆盖帧里出现的字段**, 其余保持当前值。
  /// 覆盖层(Windows 管道 / macOS FRB)可能只推局部字段(如只推 shortcuts /
  /// infiniteCanvas), 若对缺失字段用 false 兜底, 一次推送就会把面板其它开关
  /// 全部清成关、tab 数也算错。
  void _onSettingsChanged(Map<dynamic, dynamic> s) {
    _settingsLoadGen++; // 服务端推送视为最新状态, 作废在途的旧加载
    if (!mounted) return;
    final wasInfinite = _infiniteCanvas;
    final wasFreeTab = _showFreeCanvas;
    setState(() => _mergeSettingsSnapshot(s, full: false));
    // 画布模式被外部改(快捷键切自由涂鸦/覆盖层推设置)时, TabBar 选中项必须
    // 跟着走 —— 否则"玻璃在自由涂鸦、面板停在活页本", 用户再点活页本就会
    // 把自由涂鸦关掉(用户报的严重错位)。
    if (wasInfinite != _infiniteCanvas || wasFreeTab != _showFreeCanvas) {
      _syncTabToCanvasMode(force: true);
    }
    _applyLanguage(_language);
  }

  /// 把一份设置快照合并进 [_settingValues]。
  ///
  /// - 键集合/JSON 类型按 uiConfig.settings(后端登记表契约)走, 不再逐键写;
  /// - 只覆盖快照里**出现**的键(局部推送语义);
  /// - full = 完整快照(首帧 getSettings): 文本输入框跟随回显; 推送则不覆盖
  ///   用户正在编辑的内容。
  void _mergeSettingsSnapshot(Map<dynamic, dynamic> s, {required bool full}) {
    if (uiConfig.settings.isEmpty) {
      // 平台契约还没到(Windows 首连/降级): 按当前值的类型做同样的宽容解析,
      // 保留旧的"推送立即生效"行为; 契约到达后再按登记表类型归一。
      for (final key in _settingValues.keys.toList()) {
        if (!s.containsKey(key)) continue;
        if (!full && _textSettingKeys.contains(key)) continue;
        final value = _coerceByCurrentType(key, s[key]);
        if (value != null) _settingValues[key] = value;
      }
    } else {
      for (final spec in uiConfig.settings) {
        if (!uiConfig.settingAvailable(spec)) continue;
        if (!s.containsKey(spec.key)) continue;
        if (!full && _textSettingKeys.contains(spec.key)) continue;
        final value = _coerceSetting(spec.kind, s[spec.key]);
        if (value != null) _settingValues[spec.key] = value;
      }
    }
    if (s.containsKey('chatHasPassword')) {
      _chatHasPassword = _b(s['chatHasPassword']);
    }
    if (s.containsKey('shortcuts')) {
      _shortcutsRaw = s['shortcuts'];
      _shortcuts = _decodeShortcuts(_shortcutsRaw);
    }
    if (full) _syncTextControllers();
    // 归一化: 面板档位是合法子集, 旧库/旧端可能存区间内其它值。
    _settingValues['bubbleReplaySpeed'] = _vInt('bubbleReplaySpeed', 1).clamp(1, 8);
  }

  /// 按登记表类型把 JSON 标量转成面板值; 类型不符返回 null(跳过该键)。
  dynamic _coerceSetting(String kind, dynamic v) {
    switch (kind) {
      case 'bool':
        return v == null ? null : _b(v);
      case 'int':
        return v is num ? v.toInt() : null;
      case 'float':
        return v is num ? v.toDouble() : null;
      case 'str':
        return v is String ? v : null;
      default:
        return null;
    }
  }

  /// 契约缺失时的宽容解析: 以当前值的运行时类型为准(旧行为: bool 字段
  /// 历史上可能发成 int 0/1)。
  dynamic _coerceByCurrentType(String key, dynamic v) {
    final cur = _settingValues[key];
    if (cur is bool) return v == null ? null : _b(v);
    if (cur is int) return v is num ? v.toInt() : null;
    if (cur is double) return v is num ? v.toDouble() : null;
    if (cur is String) return v is String ? v : null;
    return v;
  }

  /// 完整快照到达后把文本设置回显到输入框(登录/NAS 备份)。
  void _syncTextControllers() {
    for (final key in _textSettingKeys) {
      final controller = _settingTextController(key);
      final text = _vStr(key);
      if (controller.text != text) controller.text = text;
    }
  }

  /// 拉取一次后端平台 UI 配置(macOS 立即;Windows 管道连上后由 onConnected
  /// 再拉一次)。失败时保留 [UiConfig.fallback] 只打日志 —— 面板必须能渲染,
  /// 不能因为平台配置取不到就白屏(widget 测试同样走这条降级路径)。
  Future<void> _loadUiConfig() async {
    final UiConfig cfg;
    try {
      cfg = UiConfig.fromJson(await _bridge.uiConfig());
      if (cfg.protocolVersion != kPanelProtocolVersion) {
        throw FormatException(
            '面板协议版本不一致: 面板 v$kPanelProtocolVersion, 后端 v${cfg.protocolVersion}'
            '(Windows 独立面板 exe 与覆盖层新旧混跑; 重启面板或重装即可)');
      }
    } catch (e) {
      // 管道没连上/后端版本不匹配:保留 fallback,面板照常渲染
      debugPrint('[UiConfig] load failed — using fallback defaults: $e');
      return;
    }
    uiConfigNotifier.value = cfg;
    if (!mounted) return;
    // 面板动作表必须与后端 actionOrder 一致(两端 JSON 共用同一份 id 表)。
    // 放在 catch 外面: 不一致属于代码缺陷, 断言/日志不能被降级逻辑吞掉。
    _validateShortcutActionTable(cfg);
    // 平台默认表可能刚变(Windows 首连):用设置原文重新补默认绑定
    setState(() {
      _shortcuts = _decodeShortcuts(_shortcutsRaw);
    });
  }

  /// 设置契约自检: 后端下发的键必须覆盖 uiConfig.settings 中当前平台可用的
  /// 全部项。少一项 = 该平台漏实现/漏下发 —— 这是"macOS 定义了、Windows 没实现"
  /// 最早的信号, 在面板启动时就打日志并 debug 断言, 不等真机手点才发现。
  void _validateSettingsContract(Map<dynamic, dynamic> settings) {
    final specs = uiConfig.settings;
    if (specs.isEmpty || settings.isEmpty) return;
    final missing = <String>[
      for (final s in specs)
        if (uiConfig.settingAvailable(s) && !settings.containsKey(s.key)) s.key,
    ];
    if (missing.isEmpty) return;
    debugPrint('[UiConfig] 后端设置快照缺少契约项: $missing');
    assert(missing.isEmpty, '后端设置快照缺少契约项: $missing');
  }

  Future<void> _loadSettings() async {
    final gen = ++_settingsLoadGen;
    try {
      final settings = await _bridge.getSettings();
      // 等待期间用户改过设置 / 收到推送 → 丢弃这份过期快照
      if (gen != _settingsLoadGen) return;
      if (!mounted || settings.isEmpty) {
        // Windows 管道未就绪时 getSettings 返回空:稍后重试
        if (mounted) {
          _reloadTimer = Timer(const Duration(seconds: 2), _loadSettings);
        }
        return;
      }
      // 快照契约自检(只在完整快照上做; 覆盖层局部推送不适用)。
      _validateSettingsContract(settings);
      final wasInfinite = _infiniteCanvas;
      final wasFreeTab = _showFreeCanvas;
      setState(() => _mergeSettingsSnapshot(settings, full: true));
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
      debugPrint('[Settings] set $key failed: $e');
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
      value: _vBool('chatIntegration'),
      dense: true,
      contentPadding: EdgeInsets.zero,
      onChanged: (v) => _writeSetting('chatIntegration', v),
    ));
    if (!_vBool('chatIntegration')) {
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