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

  test('typed uiConfig decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(14),
      uiConfigResponse: pb.UiConfigResponse(
        config: pb.UiConfig(
          platform: 'windows',
          isWindows: true,
          protocolVersion: 1,
        ),
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'uiConfig_response');
    expect(msg.data['platform'], 'windows');
    expect(msg.data['isWindows'], true);
  });

  test('typed canvas overview decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(13),
      canvasOverviewResponse: pb.CanvasOverviewResponse(
        png: [1, 2, 3],
        rect: [1.0, 2.0, 3.0, 4.0],
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'canvasOverview_response');
    expect(msg.data['png'], [1, 2, 3]);
    expect(msg.data['rect'], [1.0, 2.0, 3.0, 4.0]);
  });

  test('typed page op response decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(12),
      pageOpResponse: pb.PageOpResponse(
        kind: 'lassoSelect_response',
        ids: pb.Int64List(values: [Int64(3), Int64(4)]),
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'lassoSelect_response');
    expect(msg.data['ids'], [3, 4]);
  });

  test('typed bubble messages decodes', () {
    final frame = pb.Frame(
      protocolVersion: 1,
      requestId: Int64(5),
      bubbleMessages: pb.BubbleMessages(
        device: 'dev',
        messages: [
          pb.BubbleMessage(
            id: Int64(1),
            tsMs: Int64(2),
            device: 'dev',
            strokes: [
              pb.BubbleStroke(
                rgb: 0xff0000,
                widthScale: 2.0,
                gapMs: Int64(0),
                points: [pb.BubblePoint(x: 1.0, y: 2.0, width: 3.0)],
              ),
            ],
          ),
        ],
      ),
    );
    final msg = PanelStreamDecoder().feed(encodeFrame(frame)).single;
    expect(msg.kind, 'bubbleMessages_response');
    expect(msg.data['device'], 'dev');
    expect(msg.data['messages'][0]['strokes'][0]['pts'], [1.0, 2.0, 3.0]);
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
