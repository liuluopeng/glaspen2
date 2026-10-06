// i18n 冒烟: 同一组件树在 zh/en 下渲染各自语言的字符串。
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/l10n/app_localizations.dart';
import 'package:glaspen2_settings/main.dart' show SettingsPage;

Widget _host(Locale locale) => MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      locale: locale,
      home: const Scaffold(body: SettingsPage()),
    );

void main() {
  testWidgets('zh(默认模板)与 en 各自渲染', (tester) async {
    await tester.pumpWidget(_host(const Locale('zh')));
    await tester.pump();
    expect(find.text('设置'), findsWidgets);
    expect(find.text('Settings'), findsNothing);

    await tester.pumpWidget(_host(const Locale('en')));
    await tester.pump();
    expect(find.text('Settings'), findsWidgets);
    expect(find.text('设置'), findsNothing);
  });
}
