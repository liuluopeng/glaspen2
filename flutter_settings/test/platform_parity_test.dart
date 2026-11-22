// 平台一致性闸门: Flutter 侧不得再出现按平台分支的行为代码。
//
// 背景: 设置面板的所有平台差异(默认键位/保留组合/强制绑定/修饰键名称/
// 键盘示意图行/字号/功能可用性)都改由后端 JSON 描述
// (crates/glaspen-core/src/ui.rs);Flutter 只有一份代码。本测试递归扫描
// lib/**/*.dart:
//   1. 只允许 main.bridge.dart(桥工厂)出现 `Platform.` 文本,其余文件一旦
//      出现就是未来平台行为分叉的起点 —— 直接失败;
//   2. 禁止再出现「管道未接入」式静默空壳:Windows 未接入的功能必须由
//      uiConfig.features 隐藏入口,桥层未实现的方法抛 UnsupportedError。
//
// 生成代码(lib/src/rust/**)与 l10n 生成产物(lib/l10n/**)不参与扫描。
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/main.dart' show UiConfig;

/// 唯一允许按平台选择 bridge 的文件(工厂必须在运行时二选一:FRB / 命名管道)。
const Set<String> _bridgeFactoryAllowlist = {'lib/main.bridge.dart'};

/// 生成/第三方产物目录:不参与自动扫描。
const List<String> _skippedPrefixes = ['lib/src/rust/', 'lib/l10n/'];

/// 当前包根目录(flutter test 的 cwd 通常是包根;从任意子目录起跑也能找到)。
Directory _packageRoot() {
  var dir = Directory.current;
  for (var i = 0; i < 6; i++) {
    if (File('${dir.path}/pubspec.yaml').existsSync() &&
        Directory('${dir.path}/lib').existsSync()) {
      return dir;
    }
    if (dir.parent.path == dir.path) break;
    dir = dir.parent;
  }
  fail('找不到 flutter_settings 包根目录(cwd=${Directory.current.path})');
}

/// 递归列出 lib 下所有参与扫描的 .dart 文件,路径统一成 / 分隔。
List<File> _scannedLibFiles(Directory root) {
  final files = <File>[];
  for (final entity in Directory('${root.path}/lib').listSync(recursive: true)) {
    if (entity is! File || !entity.path.endsWith('.dart')) continue;
    final path = entity.path.replaceAll(Platform.pathSeparator, '/');
    if (_skippedPrefixes.any(path.contains)) continue;
    files.add(entity);
  }
  return files;
}

String _relative(File file) {
  final path = file.path.replaceAll(Platform.pathSeparator, '/');
  final idx = path.indexOf('/lib/');
  return idx < 0 ? path : path.substring(idx + 1);
}

void main() {
  test('平台分支只允许出现在 bridge 工厂', () {
    final root = _packageRoot();
    final offenders = <String>[];
    for (final file in _scannedLibFiles(root)) {
      final rel = _relative(file);
      if (_bridgeFactoryAllowlist.contains(rel)) continue;
      final lines = file.readAsLinesSync();
      for (var i = 0; i < lines.length; i++) {
        if (lines[i].contains('Platform.')) {
          offenders.add('$rel:${i + 1}: ${lines[i].trim()}');
        }
      }
    }
    expect(
      offenders,
      isEmpty,
      reason: 'Flutter 侧出现平台分支;平台差异必须由后端 uiConfig(ui.rs)下发。'
          '请改用 uiConfig 字段,或(仅桥工厂)更新 allowlist:\n'
          '${offenders.join('\n')}',
    );
  });

  test('allowlist 恰好是 bridge 工厂一个文件,且该文件确实是平台工厂', () {
    expect(_bridgeFactoryAllowlist, hasLength(1));
    expect(_bridgeFactoryAllowlist, contains('lib/main.bridge.dart'));
    final root = _packageRoot();
    final bridge = File('${root.path}/lib/main.bridge.dart');
    expect(bridge.existsSync(), isTrue);
    expect(bridge.readAsStringSync(), contains('Platform.'));
  });

  test('lib 中不再有「管道未接入」静默空壳', () {
    final root = _packageRoot();
    final hits = <String>[];
    for (final file in _scannedLibFiles(root)) {
      final rel = _relative(file);
      final lines = file.readAsLinesSync();
      for (var i = 0; i < lines.length; i++) {
        if (lines[i].contains('管道未接入')) {
          hits.add('$rel:${i + 1}: ${lines[i].trim()}');
        }
      }
    }
    expect(hits, isEmpty,
        reason: '发现静默空壳(Windows 未接入必须由 features 隐藏入口,'
            '桥层未实现的方法抛 UnsupportedError):\n${hits.join('\n')}');
  });

  group('UiConfig 契约', () {
    test('fallback 保持 macOS 行为(widget 测试与管道未连上时依赖它)', () {
      final f = UiConfig.fallback();
      expect(f.isWindows, isFalse);
      expect(f.platform, 'macos');
      expect(f.fontScale, 1.4);
      expect(f.chordSeparator, '');
      expect(f.chordNames['cmd'], '⌘');
      expect(f.features['invertInk'], isTrue);
      expect(f.features['pageFlipEffect'], isTrue);
      expect(f.features['pageExport'], isTrue);
      expect(f.features['ocrSearch'], isTrue);
      expect(f.holdHotkeys['gif'], 'cmd+ctrl+R');
      expect(f.defaults.length, f.actionOrder.length);
      expect(f.defaults['undo'], 'cmd+ctrl+Z');
      expect(f.forceBound, isEmpty);
      expect(f.modifierRow.left, hasLength(3));
      expect(f.modifierRow.space, hasLength(1));
      expect(f.modifierRow.right, hasLength(3));
    });

    test('fromJson 接受后端 Windows 形状(features 全 false)', () {
      final cfg = UiConfig.fromJson(<String, dynamic>{
        'platform': 'windows',
        'isWindows': true,
        'fontScale': 1.0,
        'chordSeparator': '+',
        'chordNames': {'cmd': 'Win', 'ctrl': 'Ctrl'},
        'modifierWord': 'Alt',
        'messages': {
          'requireModifier': {
            'zh': '需要同时按 Ctrl 或 Alt',
            'en': 'Must include Ctrl or Alt',
          },
          'reserved': {'zh': '保留', 'en': 'Reserved'},
        },
        'reserved': ['ctrl+alt+R'],
        'forceBound': ['settings', 'quit'],
        'defaults': {'undo': 'ctrl+alt+Z'},
        'actionOrder': ['undo'],
        'holdHotkeys': {
          'gif': 'ctrl+alt+R',
          'draft': 'ctrl+alt+2',
          'send': 'ctrl+alt+3',
        },
        'features': {
          'invertInk': false,
          'pageFlipEffect': false,
          'pageExport': false,
          'ocrSearch': false,
        },
        'modifierRow': {
          'left': [
            {'name': 'ctrl', 'label': 'ctrl', 'width': 1.25, 'modifier': true},
          ],
          'space': [
            {'name': 'space', 'label': 'Space', 'width': 5.0, 'modifier': false},
          ],
          'right': <dynamic>[],
        },
      });
      expect(cfg.isWindows, isTrue);
      expect(cfg.chordSeparator, '+');
      expect(cfg.modifierWord, 'Alt');
      expect(cfg.features['invertInk'], isFalse);
      expect(cfg.features['pageExport'], isFalse);
      expect(cfg.forceBound, contains('settings'));
      expect(cfg.modifierRow.left.single.name, 'ctrl');
      expect(cfg.modifierRow.left.single.modifier, isTrue);
      expect(cfg.modifierRow.space.single.label, 'Space');
      expect(cfg.messages['requireModifier']?['en'], 'Must include Ctrl or Alt');
    });

    test('fromJson 拒绝空配置(管道未就绪 → 调用方回退 fallback)', () {
      expect(() => UiConfig.fromJson(<String, dynamic>{}),
          throwsFormatException);
    });
  });
}
