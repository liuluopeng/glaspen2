// 应用构建冒烟测试:设置页能正常挂载(macOS 通道在本机不可用时走降级桥)。
import 'package:flutter_test/flutter_test.dart';

import 'package:glaspen2_settings/main.dart';

void main() {
  testWidgets('settings page builds', (WidgetTester tester) async {
    await tester.pumpWidget(const GlaspenSettingsApp());
    // 管道桥有常驻的连接重试/读取定时器,pumpAndSettle 不会结束,用固定 pump
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 1));
    // 顶部模式 tab: 设置/活页本恒在; 自由涂鸦由
    // 「显示自由涂鸦画布」开关控制(默认关, 测试环境无设置加载 → 关)
    expect(find.text('设置'), findsOneWidget);
    expect(find.text('活页本'), findsOneWidget);
    expect(find.text('自由涂鸦'), findsNothing);
  });
}
