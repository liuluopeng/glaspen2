import 'package:fixnum/fixnum.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/src/proto/glaspen/ipc/v1/ipc.pb.dart' as pb;
import 'package:glaspen2_settings/src/proto/panel_wire.dart';

void main() {
  test('SettingValue protobuf roundtrip', () {
    for (final value in <dynamic>[
      true,
      false,
      25,
      3.5,
      'zh',
      [1, 2, 3],
      {'theme': 'light', 'scale': 2.5},
    ]) {
      final bytes = encodeSettingValue(value);
      final decoded = settingValueFromDart(value);
      expect(decoded.writeToBuffer(), bytes);
    }
  });

  test('typed page thumbnails decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(11),
      pageThumbnailsResponse: pb.PageThumbnailsResponse(
        thumbnails: [
          pb.PageThumbnail(id: Int64(1), png: [1, 2, 3]),
          pb.PageThumbnail(id: Int64(2), png: [9]),
        ],
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'getPageThumbnails_response');
    expect(msg.data['entries'][0]['id'], 1);
    expect(msg.data['entries'][1]['png'], [9]);
  });

  test('typed page list decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(9),
      pageListResponse: pb.PageListResponse(
        pages: [
          pb.PageSummary(id: Int64(1), width: 1920, height: 1080),
          pb.PageSummary(id: Int64(2), width: 1280, height: 720),
        ],
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'listPages_response');
    expect(msg.requestId, 9);
    expect(msg.data['data'][0]['id'], 1);
    expect(msg.data['data'][1]['w'], 1280);
  });

  test('typed settings snapshot decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(3),
      settingsSnapshotResponse: pb.SettingsSnapshotResponse(
        snapshot: pb.SettingsSnapshot(
          settings: pb.AppSettings(grid: true, gridSize: 80, language: 'zh'),
        ),
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'getSettings_response');
    expect(msg.requestId, 3);
    expect(msg.data['grid'], true);
    expect(msg.data['gridSize'], 80.0);
    expect(msg.data['language'], 'zh');
  });

  test('panel frame response decodes data', () {
    final bytes = encodeResponseForTest(
      7,
      'onSettingsChanged',
      {'grid': true, 'scale': 2.0},
    );
    final decoded = PanelStreamDecoder().feed(bytes);
    expect(decoded.length, 1);
    expect(decoded.first.kind, 'onSettingsChanged');
    expect(decoded.first.requestId, 7);
    expect(decoded.first.data['grid'], true);
  });
}
