// schema 渲染器闸门(D11b 第二步)。
//
// 用 core 生成的真实 uiConfig 固定样例(test/fixtures/ui_config_{macos,windows}.json,
// core 侧 ui::tests::flutter_schema_fixtures_match_core_output 保证不漂移)注入
// uiConfigNotifier, 断言 schema 生成的表单与旧手写表单逐项一致:
// 分区/顺序/标签/控件类型/档位/取值/条件显示/平台与 features 门控。
//
// 测试环境连不上桥(macOS FRB 符号不可用), 设置值保持默认 —— 与旧表单在
// 同样降级条件下的默认渲染应完全一致。
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:glaspen2_settings/main.dart'
    show GlaspenSettingsApp, UiConfig, uiConfigNotifier;

/// flutter test 的 cwd 一般是包根; 从任意子目录起跑也能找到 fixtures。
File _fixtureFile(String name) {
  var dir = Directory.current;
  for (var i = 0; i < 6; i++) {
    final f = File('${dir.path}/test/fixtures/ui_config_$name.json');
    if (f.existsSync()) return f;
    if (dir.parent.path == dir.path) break;
    dir = dir.parent;
  }
  fail('找不到 test/fixtures/ui_config_$name.json (cwd=${Directory.current.path})');
}

UiConfig _fixture(String name) =>
    UiConfig.fromJson(jsonDecode(_fixtureFile(name).readAsStringSync()) as Map<String, dynamic>);

void _desktopSurface(WidgetTester tester) {
  tester.view.physicalSize = const Size(1600, 1400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
}

Future<void> _pumpPanel(WidgetTester tester, UiConfig cfg) async {
  _desktopSurface(tester);
  uiConfigNotifier.value = cfg;
  addTearDown(() => uiConfigNotifier.value = UiConfig.fallback());
  await tester.pumpWidget(const GlaspenSettingsApp(locale: Locale('zh')));
  await tester.pump(const Duration(seconds: 1));
}

/// 设置页里所有 Text 的文案(用于"没有 [key] 占位"之类的断言)。
Set<String> _texts(WidgetTester tester) => {
      for (final e in tester.widgetList<Text>(find.byType(Text)))
        if (e.data != null) e.data!,
    };

Finder _switchOf(String label) => find.descendant(
      of: find.ancestor(
          of: find.text(label), matching: find.byType(SwitchListTile)),
      matching: find.byType(Switch),
    );

Future<void> _toggle(WidgetTester tester, String label) async {
  final f = _switchOf(label);
  await tester.ensureVisible(f);
  await tester.pump();
  await tester.tap(f);
  await tester.pump();
}

/// 档位文案是 SegmentedButton 的后代, 用任一档位文案定位整个控件。
SegmentedButton<Object> _segWithOption(WidgetTester tester, String optionText) {
  final f = find.ancestor(
    of: find.text(optionText),
    matching: find.byType(SegmentedButton<Object>),
  );
  expect(f, findsOneWidget, reason: '找不到含档位 "$optionText" 的分段按钮');
  return tester.widget<SegmentedButton<Object>>(f);
}

void main() {
  group('schema 渲染(真实 macOS uiConfig)', () {
    testWidgets('分区/标签/控件与旧手写表单一致', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      final texts = _texts(tester);

      // 分区标题(顺序见下方的位置断言)。
      for (final t in ['笔', '画布外观', '系统与辅助', '界面语言', '关于']) {
        expect(texts, contains(t), reason: '缺分区标题 $t');
      }
      // 已迁移分区的控件标签。
      for (final t in [
        '颜色',
        '粗细',
        '自由涂鸦模式',
        '气泡消息',
        '显示网格',
        '分栏',
        '网格跟随涂鸦',
        '磨砂玻璃跟随涂鸦',
        '笔迹描边',
        '软阴影',
        '反色突出',
        '压力监控',
        '开机自启',
      ]) {
        expect(texts, contains(t), reason: '缺控件标签 $t');
      }
      // 未迁移分区仍由旧 builder 渲染(第二步的兜底;第三步全部接管)。
      // 注意: shortcuts 分区标题此时仍是旧硬编码「快捷键」(第三步改用
      // schema 的 hotkeys/hotkeysCtrlAlt 键)。
      for (final t in ['活页本外观', '录制与导出', '数据备份', '涂鸦身份', '快捷键']) {
        expect(texts, contains(t), reason: '缺分区标题 $t');
      }
      // 语言档位(select 通用渲染)。
      expect(texts, containsAll(<String>['中文', 'English']));
      // l10n 键全部解析到了, 没有 [key] 占位。
      expect(
        texts.where((t) => t.startsWith('[') && t.endsWith(']')),
        isEmpty,
        reason: 'schema 文案键没解析: ${texts.where((t) => t.startsWith('[')).toList()}',
      );
      // 分区顺序 = schema 顺序(位置单调递增)。
      double yOf(String t) => tester.getTopLeft(find.text(t).first).dy;
      final order = ['笔', '画布外观', '系统与辅助', '界面语言', '关于'];
      for (var i = 1; i < order.length; i++) {
        expect(yOf(order[i]), greaterThan(yOf(order[i - 1])),
            reason: '分区顺序漂移: ${order[i]}');
      }
      // 简短首屏: 笔在最上, 关于在最下。
      expect(yOf('笔'), lessThan(yOf('画布外观')));
    });

    testWidgets('条件显示: showBubbleTab 打开后才出现气泡重播控件', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      // 默认关: 重播控件不渲染。
      expect(find.text('重播速度'), findsNothing);
      expect(find.text('播完停在最后一帧'), findsNothing);
      expect(find.text('打开时自动重播最新'), findsNothing);
      expect(find.text('写完自动新建一页'), findsNothing);
      // 打开气泡 tab 开关。
      await _toggle(tester, '气泡消息');
      expect(find.text('重播速度'), findsOneWidget);
      expect(find.text('播完停在最后一帧'), findsOneWidget);
      expect(find.text('打开时自动重播最新'), findsOneWidget);
      expect(find.text('写完自动新建一页'), findsOneWidget);
      // 重播速度是 1/2/4 档的分段按钮(旧表单一致)。
      final seg = _segWithOption(tester, '2×');
      expect(seg.segments.map((s) => s.value), [1, 2, 4]);
      expect(seg.selected, {1});
      // 再点一次: 收起。
      await _toggle(tester, '气泡消息');
      expect(find.text('重播速度'), findsNothing);
    });

    testWidgets('条件显示: grid 打开后才出现网格尺寸; 分栏始终显示', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      expect(find.text('网格大小'), findsNothing);
      // 分栏不依赖显示网格(旧行为: 两者并列)。
      expect(find.text('分栏'), findsOneWidget);
      final divider = _segWithOption(tester, '左右两栏');
      expect(divider.segments.map((s) => s.value), [0, 1, 2, 3, 4]);
      expect(
        divider.segments.map((s) => ((s.label as Text).data)).toList(),
        ['无', '左右两栏', '上下两栏', '九宫格', '康奈尔'],
      );
      await _toggle(tester, '显示网格');
      expect(find.text('网格大小'), findsOneWidget);
      final size = _segWithOption(tester, '40');
      expect(size.segments.map((s) => s.value), [20, 40, 80]);
      // 默认值 40 高亮。
      expect(size.selected, {40});
    });

    testWidgets('反色帧率: 启用反色后才出现, 且是 10/30/60/100Hz', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      expect(find.text('10Hz'), findsNothing);
      await _toggle(tester, '反色突出');
      expect(find.text('10Hz'), findsOneWidget);
      expect(find.text('30Hz'), findsOneWidget);
      expect(find.text('60Hz'), findsOneWidget);
      expect(find.text('100Hz'), findsOneWidget);
      // 帧率上限的脚注(description 作脚注, 不是 subtitle)。
      expect(find.textContaining('反色追踪帧率上限'), findsOneWidget);
    });

    testWidgets('共享画布整节受 chatIntegration 门控', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      expect(find.text('共享画布'), findsNothing);
      expect(find.text('实时发送涂鸦'), findsNothing);
      // 总开关在「涂鸦身份」分区(未迁移, 旧 builder): 打开后共享画布整节出现。
      await _toggle(tester, '启用手写消息与共享');
      expect(find.text('共享画布'), findsOneWidget);
      expect(find.text('实时发送涂鸦'), findsOneWidget);
    });

    testWidgets('features 门控: Windows 配置不渲染反色/翻页动效', (tester) async {
      await _pumpPanel(tester, _fixture('windows'));
      final texts = _texts(tester);
      expect(texts, isNot(contains('反色突出')), reason: 'Windows 没有反色可用项');
      expect(texts, isNot(contains('翻页动效')), reason: 'Windows 没有翻页动效');
      // 通用控件仍正常。
      expect(texts, containsAll(<String>['自由涂鸦模式', '显示网格', '界面语言']));
      // 没有 [key] 占位。
      expect(texts.where((t) => t.startsWith('[') && t.endsWith(']')), isEmpty);
    });

    testWidgets('「更多设置」折叠: 外观等收起, 关于始终显示', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      expect(find.text('画布外观'), findsOneWidget);
      await tester.tap(find.text('收起更多设置'));
      await tester.pump();
      expect(find.text('画布外观'), findsNothing);
      expect(find.text('系统与辅助'), findsNothing);
      // 关于区在折叠之外(旧行为)。
      expect(find.text('关于'), findsOneWidget);
      // 笔(首屏)始终显示。
      expect(find.text('颜色'), findsOneWidget);
      await tester.tap(find.text('更多设置'));
      await tester.pump();
      expect(find.text('画布外观'), findsOneWidget);
    });
  });

  group('与旧手写表单一致性', () {
    /// 降级(无 schema)走旧手写表单; 已迁移分区的标签集合必须与 schema 版一致。
    testWidgets('迁移分区在旧表单/schema 表单里渲染出同一组标签', (tester) async {
      const migrated = [
        '颜色', '粗细', '自由涂鸦模式', '气泡消息', '显示网格', '分栏',
        '网格跟随涂鸦', '磨砂玻璃跟随涂鸦', '笔迹描边', '软阴影', '反色突出',
        '压力监控', '开机自启', '中文', 'English',
      ];
      await _pumpPanel(tester, UiConfig.fallback());
      final legacy = _texts(tester);
      for (final t in migrated) {
        expect(legacy, contains(t), reason: '旧表单缺 $t');
      }
      // 条件项默认都不显示(两边默认一致)。
      expect(legacy, isNot(contains('重播速度')));
      expect(legacy, isNot(contains('网格大小')));

      // 切到 schema 版: 同一组标签都在。
      await _pumpPanel(tester, _fixture('macos'));
      final schema = _texts(tester);
      for (final t in migrated) {
        expect(schema, contains(t), reason: 'schema 表单缺 $t');
      }
      expect(schema, isNot(contains('重播速度')));
      expect(schema, isNot(contains('网格大小')));
    });

    testWidgets('无 schema 时整页回退旧表单(保证后端未连时仍可操作)', (tester) async {
      await _pumpPanel(tester, UiConfig.fallback());
      expect(find.text('界面语言'), findsOneWidget);
      expect(find.text('活页本外观'), findsOneWidget);
      expect(find.text('录制与导出'), findsOneWidget);
    });
  });
}
