// 测试共用: 从 core 生成的固定样例加载真实 uiConfig(schema)。
//
// 面板在 `flutter test` 里连不上桥(FRB 符号不可用), 设置页需要后端 schema
// 才能渲染; 样例由 core 输出生成, core 测试保证不漂移(见 fixtures/README.md)。
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:glaspen2_settings/main.dart' show UiConfig, uiConfigNotifier;

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

/// 读 fixture。默认 macOS。
UiConfig uiConfigFixture([String name = 'macos']) => UiConfig.fromJson(
    jsonDecode(_fixtureFile(name).readAsStringSync()) as Map<String, dynamic>);

/// 注入 fixture 配置并注册恢复; 调用方随后 pumpWidget。
void useUiConfigFixture([String name = 'macos']) {
  uiConfigNotifier.value = uiConfigFixture(name);
  addTearDown(() => uiConfigNotifier.value = UiConfig.fallback());
}
