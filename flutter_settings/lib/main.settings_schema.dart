part of 'main.dart';

// ── 设置表单 schema:模型 + 通用渲染器(D11b)──
//
// 后端 uiConfig 下发:
// - settingsSections: 分区(显示顺序/标题 l10n 键/advanced/整节条件);
// - settings[].form: 每个设置项的控件(类型/布局/档位/单位/范围/l10n 键/
//   features 门控/设置值条件);
// - settingsBlocks: 分区内非设置项的自定义区块(命令按钮/登录/关于)。
//
// 渲染规则 = 旧手写表单的语义:
// 1. 分区按 settingsSections 顺序; advanced=false 常显, advanced=true 收进
//    「更多设置」开关并按 _showAdvanced 折叠;
// 2. 分区内控件(与自定义区块)按 order 排序;
// 3. 丢弃当前平台不可用(registry mac/win)、features 关闭、设置值条件不成立
//    的控件;
// 4. 控件全部通用渲染, 不再按设置键写 widget —— 新增设置项只改 core registry
//    + 本文件不需要动(除非新增控件类型/布局)。
//
// 迁移期(第二步)只接管已迁移分区;未迁移分区仍用旧 builder 兜底, uiConfig
// 拿不到时整页回退旧表单(见 main.dart _legacySettingsChildren)。

// ── schema 模型 ─────────────────────────────────────────────

/// 一个可选档位(值 + 显示用 l10n 键; labelId 为空 = 纯数值档位)。
class UiFormOption {
  final dynamic value;
  final String labelId;
  const UiFormOption({required this.value, required this.labelId});

  static List<UiFormOption> list(dynamic raw) {
    if (raw is! List) return const [];
    final out = <UiFormOption>[];
    for (final e in raw) {
      if (e is! Map) continue;
      out.add(UiFormOption(
        value: e['value'],
        labelId: e['labelId'] is String ? e['labelId'] as String : '',
      ));
    }
    return out;
  }
}

/// 设置值条件 {key, equals}: 当 key 的当前值等于 equals 时成立。
class UiSettingCondition {
  final String key;
  final dynamic equals;
  const UiSettingCondition({required this.key, required this.equals});

  static UiSettingCondition? tryParse(dynamic raw) {
    if (raw is! Map) return null;
    final key = raw['key'];
    if (key is! String || key.isEmpty) return null;
    return UiSettingCondition(key: key, equals: raw['equals']);
  }

  /// 条件是否成立。数值做宽松比较(JSON int/double 与面板 int 可能不同)。
  bool holds(Map<String, dynamic> values) {
    final cur = values[key];
    final want = equals;
    if (cur is num && want is num) return cur.toDouble() == want.toDouble();
    return cur == want;
  }
}

/// 数值范围(仅作展示/校验提示; 档位以 options 为准)。
class UiFormRange {
  final double min, max, step;
  const UiFormRange(this.min, this.max, this.step);
  static UiFormRange? tryParse(dynamic raw) {
    if (raw is! Map) return null;
    final min = raw['min'], max = raw['max'], step = raw['step'];
    if (min is! num || max is! num || step is! num) return null;
    return UiFormRange(min.toDouble(), max.toDouble(), step.toDouble());
  }
}

/// 一个表单可见设置项的 schema(对应 core settings_form::FormMeta::form_json)。
class UiFormMeta {
  final String key;
  final String section;
  final int order;
  final String labelId;
  final String? descriptionId;
  final String? hintId;
  /// toggle / select / text / password / shortcuts / colorGrid / widthGrid
  final String control;
  /// switch / textField / labeledGrid / inlineSegmented / intrinsicSegmented /
  /// stackedSegmented / chips / custom
  final String layout;
  final bool showLabel;
  final bool indented;
  final String? unit;
  final UiFormRange? range;
  final List<UiFormOption> options;
  final String? visibleWhenFeature;
  final UiSettingCondition? visibleWhenSetting;
  final UiSettingCondition? enabledWhenSetting;
  final bool advanced;

  const UiFormMeta({
    required this.key,
    required this.section,
    required this.order,
    required this.labelId,
    this.descriptionId,
    this.hintId,
    required this.control,
    required this.layout,
    required this.showLabel,
    required this.indented,
    this.unit,
    this.range,
    this.options = const [],
    this.visibleWhenFeature,
    this.visibleWhenSetting,
    this.enabledWhenSetting,
    this.advanced = false,
  });

  static UiFormMeta? tryParse(dynamic raw) {
    if (raw is! Map) return null;
    final key = raw['key'];
    final section = raw['section'];
    final labelId = raw['labelId'];
    if (key is! String || key.isEmpty) return null;
    if (section is! String || section.isEmpty) return null;
    if (labelId is! String || labelId.isEmpty) return null;
    return UiFormMeta(
      key: key,
      section: section,
      order: (raw['order'] as num?)?.toInt() ?? 0,
      labelId: labelId,
      descriptionId: raw['descriptionId'] is String ? raw['descriptionId'] as String : null,
      hintId: raw['hintId'] is String ? raw['hintId'] as String : null,
      control: raw['control'] is String ? raw['control'] as String : '',
      layout: raw['layout'] is String ? raw['layout'] as String : '',
      showLabel: raw['showLabel'] != false,
      indented: raw['indented'] == true,
      unit: raw['unit'] is String ? raw['unit'] as String : null,
      range: UiFormRange.tryParse(raw['range']),
      options: UiFormOption.list(raw['options']),
      visibleWhenFeature:
          raw['visibleWhenFeature'] is String ? raw['visibleWhenFeature'] as String : null,
      visibleWhenSetting: UiSettingCondition.tryParse(raw['visibleWhenSetting']),
      enabledWhenSetting: UiSettingCondition.tryParse(raw['enabledWhenSetting']),
      advanced: raw['advanced'] == true,
    );
  }
}

/// 一个表单分区(对应 core settings_form::SECTIONS 的 sections_json 条目)。
class UiFormSection {
  final String id;
  final String labelId;
  final bool advanced;
  final int order;
  final UiSettingCondition? visibleWhenSetting;
  const UiFormSection({
    required this.id,
    required this.labelId,
    required this.advanced,
    required this.order,
    this.visibleWhenSetting,
  });

  static List<UiFormSection> list(dynamic raw) {
    if (raw is! List) return const [];
    final out = <UiFormSection>[];
    for (final e in raw) {
      if (e is! Map) continue;
      final id = e['id'];
      final labelId = e['labelId'];
      if (id is! String || id.isEmpty || labelId is! String || labelId.isEmpty) continue;
      out.add(UiFormSection(
        id: id,
        labelId: labelId,
        advanced: e['advanced'] == true,
        order: (e['order'] as num?)?.toInt() ?? out.length,
        visibleWhenSetting: UiSettingCondition.tryParse(e['visibleWhenSetting']),
      ));
    }
    return out;
  }
}

/// 分区内非设置项的自定义区块(Dart 按 id 渲染; core 只定位)。
class UiCustomBlock {
  final String id;
  final String section;
  final int order;
  final UiSettingCondition? visibleWhenSetting;
  const UiCustomBlock({
    required this.id,
    required this.section,
    required this.order,
    this.visibleWhenSetting,
  });

  static List<UiCustomBlock> list(dynamic raw) {
    if (raw is! List) return const [];
    final out = <UiCustomBlock>[];
    for (final e in raw) {
      if (e is! Map) continue;
      final id = e['id'];
      final section = e['section'];
      if (id is! String || id.isEmpty || section is! String || section.isEmpty) continue;
      out.add(UiCustomBlock(
        id: id,
        section: section,
        order: (e['order'] as num?)?.toInt() ?? 0,
        visibleWhenSetting: UiSettingCondition.tryParse(e['visibleWhenSetting']),
      ));
    }
    return out;
  }
}

/// schema 第二步已接管的分区(其余分区暂用旧 builder 兜底;第三步清空)。
/// 只用于迁移期, 不是平台分支。
const _schemaSectionMigrated = <String>{
  'pen',
  'canvasLook',
  'systemAndAssist',
  'language',
  'sharedCanvas',
};

// ── l10n 键解析 ─────────────────────────────────────────────
//
// gen-l10n 只有静态 getter, 没有"按字符串查文案"的入口; schema 给的是 ARB 键,
// 所以这里是一张**键 → getter** 的解析表。core 的测试保证所有 label_id 都在
// ARB 两个语言文件里; 本表漏键时渲染出 [键名] 占位(测试会断言不出现)。
// 新增 schema 文案键时这里要与 core settings_form 同步。

extension _SettingsSchemaUi on _SettingsPageState {
  String _l10n(String id) {
    final l = AppLocalizations.of(context)!;
    switch (id) {
      // 分区
      case 'pen': return l.pen;
      case 'canvasLook': return l.canvasLook;
      case 'notebookLook': return l.notebookLook;
      case 'recordExport': return l.recordExport;
      case 'dataBackup': return l.dataBackup;
      case 'sharedCanvas': return l.sharedCanvas;
      case 'doodleIdentity': return l.doodleIdentity;
      case 'systemAndAssist': return l.systemAndAssist;
      case 'language': return l.language;
      case 'hotkeys': return l.hotkeys;
      case 'hotkeysCtrlAlt': return l.hotkeysCtrlAlt;
      case 'about': return l.about;
      // 控件标签
      case 'color': return l.color;
      case 'width': return l.width;
      case 'freeDoodleMode': return l.freeDoodleMode;
      case 'bubbleTab': return l.bubbleTab;
      case 'bubbleReplaySpeed': return l.bubbleReplaySpeed;
      case 'bubbleReplayHoldLast': return l.bubbleReplayHoldLast;
      case 'bubbleReplayAuto': return l.bubbleReplayAuto;
      case 'bubbleAutoNewPage': return l.bubbleAutoNewPage;
      case 'showGrid': return l.showGrid;
      case 'gridSize': return l.gridSize;
      case 'columns': return l.columns;
      case 'gridFollowsDoodles': return l.gridFollowsDoodles;
      case 'glassFollowsDoodles': return l.glassFollowsDoodles;
      case 'inkOutline': return l.inkOutline;
      case 'softShadow': return l.softShadow;
      case 'invertedInk': return l.invertedInk;
      case 'invertTrackingFpsCap': return l.invertTrackingFpsCap;
      case 'style': return l.style;
      case 'pageFlipEffect': return l.pageFlipEffect;
      case 'show10NearbyPages': return l.show10NearbyPages;
      case 'truePassThroughExperimental': return l.truePassThroughExperimental;
      case 'frameRate': return l.frameRate;
      case 'resolution': return l.resolution;
      case 'speed': return l.speed;
      case 'ending': return l.ending;
      case 'nasBackupUrl': return l.nasBackupUrl;
      case 'streamDoodlesLive': return l.streamDoodlesLive;
      case 'enableHandwrittenMessagesSharing': return l.enableHandwrittenMessagesSharing;
      case 'serverAddress': return l.serverAddress;
      case 'username': return l.username;
      case 'password': return l.password;
      case 'pressureMonitor': return l.pressureMonitor;
      case 'launchAtLogin': return l.launchAtLogin;
      // 说明(subtitle / 脚注 / hint)
      case 'showTheFreeDoodle': return l.showTheFreeDoodle;
      case 'showBubbleTabSub': return l.showBubbleTabSub;
      case 'bubbleReplayHoldLastSub': return l.bubbleReplayHoldLastSub;
      case 'bubbleReplayAutoSub': return l.bubbleReplayAutoSub;
      case 'bubbleAutoNewPageSub': return l.bubbleAutoNewPageSub;
      case 'helpsAlignWhileDoodling': return l.helpsAlignWhileDoodling;
      case 'whenOnTheGrid': return l.whenOnTheGrid;
      case 'whenOnTheGlass': return l.whenOnTheGlass;
      case 'dashedContrastOutlineVisible': return l.dashedContrastOutlineVisible;
      case 'softDarkShadowUnder': return l.softDarkShadowUnder;
      case 'strokesTakeThePer': return l.strokesTakeThePer;
      case 'verticalMinimapOnThe': return l.verticalMinimapOnThe;
      case 'cardsRevealTheDesktop': return l.cardsRevealTheDesktop;
      case 'pushedToThePeer': return l.pushedToThePeer;
      case 'whenOff23': return l.whenOff23;
      case 'eGHttps192': return l.eGHttps192;
      // 档位
      case 'cRed': return l.cRed;
      case 'cOrange': return l.cOrange;
      case 'cYellow': return l.cYellow;
      case 'cGreen': return l.cGreen;
      case 'cCyan': return l.cCyan;
      case 'cBlue': return l.cBlue;
      case 'cPurple': return l.cPurple;
      case 'cPink': return l.cPink;
      case 'cWhite': return l.cWhite;
      case 'cBlack': return l.cBlack;
      case 'wUltraFine': return l.wUltraFine;
      case 'wVeryFine': return l.wVeryFine;
      case 'wFine': return l.wFine;
      case 'wMedium': return l.wMedium;
      case 'wThick': return l.wThick;
      case 'wVeryThick': return l.wVeryThick;
      case 'wExtraThick': return l.wExtraThick;
      case 'wUltraThick': return l.wUltraThick;
      case 'none': return l.none;
      case 'twoColumnsSideBy': return l.twoColumnsSideBy;
      case 'twoColumnsStacked': return l.twoColumnsStacked;
      case 'grid3x3': return l.grid3x3;
      case 'cornell': return l.cornell;
      case 'systemAnimation': return l.systemAnimation;
      case 'timeTunnel': return l.timeTunnel;
      case 'skeuoGlass': return l.skeuoGlass;
      case 'notePaper': return l.notePaper;
      case 'stopOnLastFrame': return l.stopOnLastFrame;
      case 'hold1SThen': return l.hold1SThen;
      case 'loopImmediately': return l.loopImmediately;
      case 'chinese': return l.chinese;
      case 'english': return l.english;
      default: return '[$id]';
    }
  }

  // ── 值访问 ────────────────────────────────────────────────

  dynamic _schemaValue(String key) => _settingValues[key];

  bool _schemaBool(String key) => _settingValues[key] == true;

  /// 取档位标签: 有 l10n 键用文案, 否则按单位格式化纯数值档位。
  String _schemaOptionLabel(UiFormMeta f, UiFormOption o) {
    if (o.labelId.isNotEmpty) return _l10n(o.labelId);
    final v = o.value;
    switch (f.unit) {
      case 'fps': return '$v fps';
      case 'percent': return '${((v as num) * 100).round()}%';
      case 'x': return '${v}x';
      case 'timesCross': return '$v×';
      case 'hz': return '${v}Hz';
      default: return '$v';
    }
  }

  /// 当前值在档位里的匹配项(数值宽松比较); 没有匹配时退到第一档 ——
  /// SegmentedButton 要求 selected 属于 segments, 不能把非法旧值直接塞进去。
  dynamic _schemaSelectedValue(UiFormMeta f) {
    final cur = _schemaValue(f.key);
    for (final o in f.options) {
      final v = o.value;
      if (cur is num && v is num) {
        if (cur.toDouble() == v.toDouble()) return v;
      } else if (cur == v) {
        return v;
      }
    }
    return f.options.isEmpty ? cur : f.options.first.value;
  }

  // ── 分区/控件渲染 ──────────────────────────────────────────

  /// 用 schema 渲染设置页(第二步: 只接管已迁移分区, 其余走旧 builder)。
  Widget _buildSchemaSettingsTab() {
    final sections = uiConfig.settingsSections;
    final children = <Widget>[];
    var toggleInserted = false;
    var afterToggle = false;
    for (final s in sections) {
      if (s.visibleWhenSetting != null && !s.visibleWhenSetting!.holds(_settingValues)) {
        continue;
      }
      if (s.advanced) {
        if (!toggleInserted) {
          if (children.isNotEmpty) children.add(const SizedBox(height: 16));
          children.add(_buildMoreSettingsToggle());
          toggleInserted = true;
          afterToggle = true;
        }
        if (!_showAdvanced) continue;
      }
      final widget = _schemaSectionMigrated.contains(s.id)
          ? _buildSchemaSection(s)
          : _legacySectionFor(s.id);
      if (widget == null) continue;
      if (children.isNotEmpty) {
        children.add(SizedBox(height: afterToggle && s.advanced ? 12 : 16));
      }
      afterToggle = false;
      children.add(widget);
    }
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        key: _columnKey,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: children,
      ),
    );
  }

  Widget _buildSchemaSection(UiFormSection s) {
    final items = <(int, Widget)>[];
    for (final spec in uiConfig.settings) {
      final f = spec.form;
      if (f == null || f.section != s.id) continue;
      if (!uiConfig.settingAvailable(spec)) continue;
      if (f.visibleWhenFeature != null &&
          uiConfig.features[f.visibleWhenFeature] != true) {
        continue;
      }
      final cond = f.visibleWhenSetting;
      if (cond != null && !cond.holds(_settingValues)) continue;
      items.add((f.order, _buildSchemaControl(f)));
    }
    items.sort((a, b) => a.$1.compareTo(b.$1));
    return _buildSection(
      _l10n(s.labelId),
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [for (final e in items) e.$2],
      ),
    );
  }

  Widget _buildSchemaControl(UiFormMeta f) {
    switch (f.control) {
      case 'toggle':
        return _buildSchemaToggle(f);
      case 'colorGrid':
        return _buildSchemaGrid(f, _buildColorGrid());
      case 'widthGrid':
        return _buildSchemaGrid(f, _buildWidthRow());
      case 'select':
        return _buildSchemaSelect(f);
      case 'text':
      case 'password':
        return _buildSchemaTextField(f);
      case 'shortcuts':
        // 复合控件(动作表来自 ui.rs, 录制/校验是动态逻辑)保留手写。
        return _buildShortcutEditor();
      default:
        return const SizedBox.shrink();
    }
  }

  Widget _buildSchemaToggle(UiFormMeta f) {
    final enabled = f.enabledWhenSetting?.holds(_settingValues) ?? true;
    return _tile(SwitchListTile(
      title: Text(_l10n(f.labelId), style: const TextStyle(fontSize: 15)),
      subtitle: f.descriptionId == null
          ? null
          : Text(_l10n(f.descriptionId!), style: const TextStyle(fontSize: 12)),
      value: _schemaBool(f.key),
      dense: true,
      contentPadding: EdgeInsets.zero,
      onChanged: enabled ? (v) => _writeSetting(f.key, v) : null,
    ));
  }

  Widget _buildSchemaGrid(UiFormMeta f, Widget grid) {
    final label = f.showLabel
        ? Text(
            _l10n(f.labelId),
            style: TextStyle(
                fontSize: 13, color: _inkFaint, fontWeight: FontWeight.w600),
          )
        : null;
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ?label,
          if (label != null) const SizedBox(height: 8),
          grid,
        ],
      ),
    );
  }

  Widget _buildSchemaSelect(UiFormMeta f) {
    final enabled = f.enabledWhenSetting?.holds(_settingValues) ?? true;
    final segments = <ButtonSegment<Object>>[
      for (final o in f.options)
        ButtonSegment<Object>(
          value: o.value as Object,
          label: Text(_schemaOptionLabel(f, o)),
        ),
    ];
    final selected = <Object>{_schemaSelectedValue(f) as Object};
    final button = SegmentedButton<Object>(
      segments: segments,
      selected: selected,
      showSelectedIcon: false,
      style: const ButtonStyle(
        visualDensity: VisualDensity(horizontal: -2, vertical: -2),
      ),
      onSelectionChanged: enabled
          ? (sel) {
              if (sel.isNotEmpty) _writeSetting(f.key, sel.first);
            }
          : null,
    );
    final label = f.showLabel
        ? Text(_l10n(f.labelId), style: const TextStyle(fontSize: 13))
        : null;
    final footnote = f.descriptionId == null
        ? null
        : Text(
            _l10n(f.descriptionId!),
            style: const TextStyle(fontSize: 11, color: Colors.grey),
          );

    Widget content;
    switch (f.layout) {
      case 'inlineSegmented':
        content = Row(children: [?label, const Spacer(), button]);
      case 'stackedSegmented':
        content = Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ?label,
            if (label != null) const SizedBox(height: 4),
            button,
            if (footnote != null) const SizedBox(height: 2),
            ?footnote,
          ],
        );
      default: // intrinsicSegmented(语言: 不放标签)
        content = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [?label, button],
        );
    }
    final left = f.indented ? 16.0 : 0.0;
    return Padding(
      padding: EdgeInsets.only(left: left, right: left, bottom: 4),
      child: content,
    );
  }

  Widget _buildSchemaTextField(UiFormMeta f) {
    final controller = _settingTextController(f.key);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: TextField(
        controller: controller,
        style: const TextStyle(fontSize: 13),
        autocorrect: false,
        decoration: InputDecoration(
          labelText: _l10n(f.labelId),
          hintText: f.hintId == null ? null : _l10n(f.hintId!),
          isDense: true,
          border: const OutlineInputBorder(),
        ),
        onSubmitted: (v) => _writeSetting(f.key, v.trim()),
      ),
    );
  }
}