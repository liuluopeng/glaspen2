// 应用构建冒烟测试:设置页能正常挂载(macOS 通道在本机不可用时走降级桥)。
// i18n 后测试环境 locale 默认 en_US, 固定 zh 断言中文 tab 文案。
import 'dart:ui';
import 'package:flutter_test/flutter_test.dart';

import 'package:glaspen2_settings/main.dart'
    show GlaspenSettingsApp, appLocale;

import 'ui_config_fixture.dart';

void main() {
  testWidgets('settings page builds', (WidgetTester tester) async {
    // 默认 800x600 测试视口经 macOS 1.4 缩放只剩 ~571 逻辑宽, 远窄于真实
    // 窗口; 给冒烟测试一个桌面级表面, 排除与被测目标无关的溢出断言。
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(const GlaspenSettingsApp(locale: Locale('zh')));
    // 管道桥有常驻的连接重试/读取定时器,pumpAndSettle 不会结束,用固定 pump
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 1));
    // 顶部模式 tab: 设置/活页本恒在; 自由涂鸦由
    // 「显示自由涂鸦画布」开关控制(默认关, 测试环境无设置加载 → 关)
    expect(find.text('设置'), findsOneWidget);
    expect(find.text('活页本'), findsOneWidget);
    expect(find.text('自由涂鸦'), findsNothing);
  });

  testWidgets('language selector drives locale (no system auto-detect)',
      (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    appLocale.value = const Locale('zh');
    addTearDown(() => appLocale.value = const Locale('zh'));
    useUiConfigFixture();

    await tester.pumpWidget(const GlaspenSettingsApp());
    await tester.pump(const Duration(seconds: 1));
    // 高级设置默认展开(_showAdvanced=true), 语言选择器直接可见。
    expect(find.text('界面语言'), findsOneWidget);

    appLocale.value = const Locale('en');
    await tester.pump();
    expect(find.text('Language'), findsOneWidget);
    expect(find.text('Settings'), findsWidgets);
  });

  testWidgets('Windows uiConfig 隐藏 macOS 专属行并用后端 chord 渲染按住热键',
      (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1600, 1200);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    // 真实 Windows schema 固定样例: features 全 false + 完整表单。
    useUiConfigFixture('windows');

    await tester.pumpWidget(const GlaspenSettingsApp(locale: Locale('zh')));
    await tester.pump(const Duration(seconds: 1));
    // GIF 标题 = 本地化句子 + 后端 chord 格式化(不再按平台选 ARB key)
    expect(find.text('GIF 动画 (Ctrl+Alt+R 按住录制)'), findsOneWidget);
    // features=false 的整行/整块不渲染
    expect(find.text('反色突出'), findsNothing);
    expect(find.text('翻页动效'), findsNothing);
  });
}
