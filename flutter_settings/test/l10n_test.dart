// i18n 冒烟: 同一组件树在 zh/en 下渲染各自语言的字符串。
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/l10n/app_localizations.dart';
import 'package:glaspen2_settings/main.dart' show SettingsPage;

import 'ui_config_fixture.dart';

Widget _host(Locale locale) => MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      locale: locale,
      home: const Scaffold(body: SettingsPage()),
    );

void main() {
  testWidgets('zh(默认模板)与 en 各自渲染', (tester) async {
    // 设置页由后端 schema 生成; 测试注入 core 输出的真实 macOS uiConfig。
    useUiConfigFixture();
    await tester.pumpWidget(_host(const Locale('zh')));
    await tester.pump();
    expect(find.text('设置'), findsWidgets);
    expect(find.text('Settings'), findsNothing);
    // 高级设置默认展开(_showAdvanced=true), 语言选择器直接可见
    expect(find.text('界面语言'), findsOneWidget);
    expect(find.text('中文'), findsOneWidget);

    await tester.pumpWidget(_host(const Locale('en')));
    await tester.pump();
    expect(find.text('Settings'), findsWidgets);
    expect(find.text('设置'), findsNothing);
    expect(find.text('Language'), findsOneWidget);
    expect(find.text('English'), findsOneWidget);
  });
}
