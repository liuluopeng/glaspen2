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
      // 全部分区(含第三步迁移的活页本/录制导出/数据备份/涂鸦身份/快捷键/关于)
      // 都由 schema 装配; 快捷键标题用 ARB hotkeys(⌘⌃…) 而不是硬编码。
      for (final t in [
        '活页本外观',
        '录制与导出',
        '数据备份',
        '涂鸦身份',
        '快捷键 (⌘⌃…)',
      ]) {
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

  group('第三步:剩余分区的 schema 迁移', () {
    testWidgets('活页本/录制导出/数据备份/涂鸦身份/快捷键全部由 schema 渲染', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      final texts = _texts(tester);
      for (final t in [
        // notebookLook
        '风格', '翻页动效', '显示附近 10 页', '真穿透(实验)',
        // recordExport(GIF 档位 + 体积估算 + 导出按钮)
        '帧率', '分辨率', '速度', '结束时',
        '将全部页面的笔迹导出为 PDF 保存到桌面。', '导出全部页面为 PDF',
        // dataBackup(备份按钮 + NAS 小标题/地址/按钮)
        '备份全部数据', '从最新备份恢复', 'NAS 备份', 'NAS 地址', '立即备份到 NAS',
        // shortcuts(分区标题来自 core 的平台键)
        '快捷键 (⌘⌃…)',
        // about
        '当前版本', '检查更新',
      ]) {
        expect(texts, contains(t), reason: '缺 \$t');
      }
      // GIF 档位是 chips: 10/15/20/24/30/50 fps + 分辨率 + 速度 + 结束方式。
      for (final t in ['10 fps', '50 fps', '25%', '100%', '0.5x', '20.0x', '停在最后', '停1秒后循环']) {
        expect(texts, contains(t), reason: '缺 GIF 档位 \$t');
      }
      // NAS 地址输入框有 placeholder(ARB nasUrlExample 经 hintId 渲染;
      // Material 在未聚焦时只显示 label, 所以直接检查 InputDecoration)。
      final nasField = tester.widget<TextField>(find.ancestor(
        of: find.text('NAS 地址'),
        matching: find.byType(TextField),
      ));
      expect(nasField.decoration?.hintText, 'http://192.168.1.10:8080');
      // 没有 [key] 占位。
      expect(texts.where((t) => t.startsWith('[') && t.endsWith(']')), isEmpty);
    });

    testWidgets('涂鸦身份: 总开关打开后 服务地址/用户名/密码/按钮 出现', (tester) async {
      await _pumpPanel(tester, _fixture('macos'));
      // 默认关: 只有总开关。
      expect(find.text('服务地址'), findsNothing);
      expect(find.text('用户名'), findsNothing);
      expect(find.text('密码'), findsNothing);
      await _toggle(tester, '启用手写消息与共享');
      expect(find.text('服务地址'), findsOneWidget);
      expect(find.text('用户名'), findsOneWidget);
      expect(find.text('密码'), findsOneWidget);
      expect(find.text('保存并测试登录'), findsOneWidget);
      expect(find.textContaining('作者归属'), findsOneWidget);
      // 服务器地址的 hint 来自 ARB(eGHttps192)。
      expect(
        find.ancestor(
          of: find.textContaining('https://192'),
          matching: find.byType(TextField),
        ),
        findsWidgets,
      );
    });

    testWidgets('无 schema(后端未连接)时显示未连接提示, 不再凭空造表单', (tester) async {
      await _pumpPanel(tester, UiConfig.fallback());
      expect(find.text('未连接到主程序'), findsOneWidget);
      expect(find.text('界面语言'), findsNothing);
      expect(find.text('颜色'), findsNothing);
    });
  });
}
