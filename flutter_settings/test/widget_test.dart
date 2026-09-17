// 应用构建冒烟测试:设置页能正常挂载(macOS 通道在本机不可用时走降级桥)。
import 'package:flutter_test/flutter_test.dart';

import 'package:glaspen2_settings/main.dart';

void main() {
  testWidgets('settings page builds', (WidgetTester tester) async {
    await tester.pumpWidget(const GlaspenSettingsApp());
    // 管道桥有常驻的连接重试/读取定时器,pumpAndSettle 不会结束,用固定 pump
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 1));
    // 顶部三个模式 tab(设置 / 活页本 / 自由涂鸦)
    expect(find.text('设置'), findsOneWidget);
    expect(find.text('活页本'), findsOneWidget);
    expect(find.text('自由涂鸦'), findsOneWidget);
  });
}
