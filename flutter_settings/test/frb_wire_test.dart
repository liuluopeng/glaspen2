// flutter_rust_bridge 通信冒烟测试。
//
// 直接对 cdylib(target/debug/libglaspen2.dylib)跑一遍真实的 wire 协议:
// 符号解析、dispatcher、Int64List/结构体/二进制块的编解码。App 里用的是
// ExternalLibrary.process()(Rust 静态链进主可执行文件),这里用 open() 加载
// 同一个 crate 编译出的 dylib,验证的是除"符号来自哪里"之外的全部环节。
//
// 运行:flutter test test/frb_wire_test.dart
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart' as frb;
import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/src/rust/api.dart' as rust;
import 'package:glaspen2_settings/src/rust/frb_generated.dart';

void main() {
  final lib = File('../target/debug/libglaspen2.dylib');
  final skipReason = lib.existsSync() ? null : '先构建 cdylib:cargo build';

  setUpAll(() async {
    if (skipReason != null) return;
    await RustLib.init(externalLibrary: frb.ExternalLibrary.open(lib.path));
  });

  test('listPages 往返(Vec<PageSummary>)', () async {
    final pages = await rust.listPages();
    // 测试进程没有打开数据库 → 空列表(重点是往返本身没有报错)
    expect(pages, isEmpty);
  }, skip: skipReason);

  test('pageThumbnails 往返(Int64List → 二进制块 → Vec<PageThumb>)', () async {
    final thumbs = await rust.pageThumbnails(
      ids: frb.Int64List.fromList([1, 2, 3]),
      maxSize: 280,
    );
    expect(thumbs, isEmpty);
  }, skip: skipReason);

  test('getLens 往返(结构体)', () async {
    final lens = await rust.getLens();
    expect(lens.pageId, 0);
    expect(lens.panX, 0);
    expect(lens.panY, 0);
    expect(lens.zoom, 1.0);
  }, skip: skipReason);

  test('导出/删除等写操作在无 ObjC 环境下不抛异常', () async {
    // 这些函数在 macOS 上转发给 ObjC shim;测试进程里没有初始化 AppKit,
    // 但接口本身必须能完成一次往返。
    await rust.navigateToPage(screenId: 1);
    await rust.triggerHotkey(key: 'Z');
  }, skip: skipReason);
}
