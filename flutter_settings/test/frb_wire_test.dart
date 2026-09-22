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
    // 但接口本身必须能完成一次往返 —— 挂起或 panic 都会在这里暴露,
    // 而不是等到用户在面板上点按钮。
    await rust.navigateToPage(screenId: 1);
    await rust.triggerHotkey(key: 'Z');
    expect(await rust.deletePage(screenId: 999999), isFalse); // 不存在的页
    expect(await rust.exportPdf(), isFalse); // 测试进程没有数据库
    expect(
      await rust.canvasOverview(
          w: 1024, h: 768, action: rust.CanvasAction.current),
      isNull, // 空画布
    );
    expect(
      await rust.canvasOverview(w: 1024, h: 768, action: rust.CanvasAction.home),
      isNull,
    );
  }, skip: skipReason);

  test('备份/回导接口在无数据库时优雅返回(不 panic/不挂起)', () async {
    // 真正的备份/回导逻辑由 Rust 单测覆盖(见 db::tests::test_backup_then_restore_merge);
    // 这里确认 FRB 这条路径能把错误当成结果返回, 而不是抛异常或卡住。
    final backup = await rust.backupNow();
    expect(backup.ok, isFalse);
    expect(backup.message, isNotEmpty);

    final restore = await rust.restoreLatestBackup();
    expect(restore.ok, isFalse);
    expect(restore.message, isNotEmpty);
  }, skip: skipReason);

  test('appVersion 往返(「检查更新」显示的当前版本)', () async {
    // 只测版本号这条无网络路径;真正打 GitHub 的部分由 Rust 侧
    // update::tests::test_fetch_latest_live(-- --ignored)覆盖。
    final v = await rust.appVersion();
    expect(RegExp(r'^\d+\.\d+\.\d+').hasMatch(v), isTrue,
        reason: '版本号应形如 0.5.1, 实际: "$v"');
  }, skip: skipReason);

  test('setSetting → getSettings 往返(JSON 标量必须真的被解析)', () async {
    // 回归:ObjC 侧用 NSJSONSerialization 解析 value_json 时忘了
    // NSJSONReadingFragmentsAllowed,裸标量("true"/"3"/"2.5")全部返回 nil,
    // 于是每个设置都被静默写成 false/0 —— 面板上按按钮"没反应"。
    await rust.setSetting(key: 'grid', valueJson: 'true');
    expect((await rust.getSettings())?.grid, isTrue,
        reason: 'bool 设置没有被应用(值被解析成了 nil?)');

    await rust.setSetting(key: 'grid', valueJson: 'false');
    expect((await rust.getSettings())?.grid, isFalse);

    await rust.setSetting(key: 'gifFps', valueJson: '25');
    expect((await rust.getSettings())?.gifFps, 25, reason: 'int 设置没有被应用');

    await rust.setSetting(key: 'gifSpeed', valueJson: '3.5');
    expect((await rust.getSettings())?.gifSpeed, 3.5,
        reason: 'double 设置没有被应用');

    // 解析不了的输入必须被忽略,而不是写成 0
    await rust.setSetting(key: 'gifFps', valueJson: '25');
    await rust.setSetting(key: 'gifFps', valueJson: 'not json');
    expect((await rust.getSettings())?.gifFps, 25,
        reason: '非法 value_json 不应改动已有设置');
  }, skip: skipReason);
}
