// Windows 面板命名管道的 protobuf 分帧/Struct 转换。
// 线格式: 连接开头发送 magic, 之后每帧 = u32 little-endian 长度 + Frame bytes。
import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:protobuf/well_known_types/google/protobuf/struct.pb.dart' as wkt;

import 'glaspen/ipc/v1/ipc.pb.dart' as pb;

const List<int> panelMagic = [0x47, 0x50, 0x42, 0x31]; // "GPB1"

Map<String, dynamic> bubbleMessagesToMap(pb.BubbleMessages m) {
  return <String, dynamic>{
    'device': m.device,
    'messages': [
      for (final msg in m.messages)
        {
          'id': msg.id.toInt(),
          'ts_ms': msg.tsMs.toInt(),
          'device': msg.device,
          'strokes': [
            for (final s in msg.strokes)
              {
                'rgb': s.rgb,
                'w': s.widthScale,
                'g': s.gapMs.toInt(),
                'pts': [
                  for (final p in s.points) ...[p.x, p.y, p.width],
                ],
              },
          ],
        },
    ],
  };
}

Map<String, dynamic> appSettingsToMap(pb.AppSettings s) {
  return <String, dynamic>{
    'color': s.color,
    'width': s.width,
    'launchAtLogin': s.launchAtLogin,
    'frostedGlass': s.frostedGlass,
    'grid': s.grid,
    'gridFollowStrokes': s.gridFollowStrokes,
    'glassFollowStrokes': s.glassFollowStrokes,
    'softShadow': s.softShadow,
    'invertInk': s.invertInk,
    'notebookStyle': s.notebookStyle,
    'panelTransparent': s.panelTransparent,
    'invertFps': s.invertFps,
    'pressureMonitor': s.pressureMonitor,
    'outline': s.outline,
    'infiniteCanvas': s.infiniteCanvas,
    'minimap': s.minimap,
    'gridSize': s.gridSize,
    'gridDivider': s.gridDivider,
    'flipEffect': s.flipEffect,
    'gifFps': s.gifFps,
    'gifResolution': s.gifResolution,
    'gifSpeed': s.gifSpeed,
    'gifEndMode': s.gifEndMode,
    'chatApiBase': s.chatApiBase,
    'chatUser': s.chatUser,
    'chatHasPassword': s.chatHasPassword,
    'chatIntegration': s.chatIntegration,
    'showFreeCanvas': s.showFreeCanvas,
    'showBubbleTab': s.showBubbleTab,
    'bubbleReplaySpeed': s.bubbleReplaySpeed,
    'bubbleReplayHoldLast': s.bubbleReplayHoldLast,
    'bubbleReplayAuto': s.bubbleReplayAuto,
    'bubbleAutoNewPage': s.bubbleAutoNewPage,
    'nasUrl': s.nasUrl,
    'shareCanvas': s.shareCanvas,
    'language': s.language,
    'shortcuts': s.shortcuts,
  };
}

class PanelMessage {
  const PanelMessage(this.kind, this.requestId, this.data);
  final String kind;
  final int requestId;
  final dynamic data;
}

wkt.Value jsonToProstValue(dynamic value) {
  if (value == null) {
    return wkt.Value(nullValue: wkt.NullValue.NULL_VALUE);
  }
  if (value is bool) {
    return wkt.Value(boolValue: value);
  }
  if (value is num) {
    return wkt.Value(numberValue: value.toDouble());
  }
  if (value is String) {
    return wkt.Value(stringValue: value);
  }
  if (value is Iterable) {
    return wkt.Value(
      listValue: wkt.ListValue(
        values: value.map(jsonToProstValue).toList(),
      ),
    );
  }
  if (value is Map) {
    return wkt.Value(
      structValue: wkt.Struct(
        fields: value.entries.map(
          (e) => MapEntry(e.key.toString(), jsonToProstValue(e.value)),
        ),
      ),
    );
  }
  return wkt.Value(stringValue: value.toString());
}

dynamic prostValueToJson(wkt.Value value) {
  switch (value.whichKind()) {
    case wkt.Value_Kind.nullValue:
      return null;
    case wkt.Value_Kind.numberValue:
      return value.numberValue;
    case wkt.Value_Kind.stringValue:
      return value.stringValue;
    case wkt.Value_Kind.boolValue:
      return value.boolValue;
    case wkt.Value_Kind.structValue:
      return {
        for (final e in value.structValue.fields.entries)
          e.key: prostValueToJson(e.value),
      };
    case wkt.Value_Kind.listValue:
      return value.listValue.values.map(prostValueToJson).toList();
    case wkt.Value_Kind.notSet:
      return null;
  }
}

List<int> encodeSettingValue(dynamic value) {
  final setting = settingValueFromDart(value);
  return setting.writeToBuffer();
}

pb.SettingValue settingValueFromDart(dynamic value) {
  if (value == null) {
    return pb.SettingValue(nullValue: true);
  }
  if (value is bool) {
    return pb.SettingValue(boolValue: value);
  }
  if (value is int) {
    return pb.SettingValue(intValue: Int64(value));
  }
  if (value is double) {
    return pb.SettingValue(doubleValue: value);
  }
  if (value is String) {
    return pb.SettingValue(stringValue: value);
  }
  if (value is Uint8List) {
    return pb.SettingValue(bytesValue: value);
  }
  if (value is Iterable) {
    return pb.SettingValue(
      listValue: pb.ListValue(
        values: value.map(settingValueFromDart).toList(),
      ),
    );
  }
  if (value is Map) {
    return pb.SettingValue(
      structValue: wkt.Struct(
        fields: value.entries.map(
          (e) => MapEntry(e.key.toString(), jsonToProstValue(e.value)),
        ),
      ),
    );
  }
  return pb.SettingValue(stringValue: value.toString());
}

List<int> encodeFrame(pb.Frame frame) {
  final payload = frame.writeToBuffer();
  final out = Uint8List(4 + payload.length);
  final len = payload.length;
  out[0] = len & 0xff;
  out[1] = (len >> 8) & 0xff;
  out[2] = (len >> 16) & 0xff;
  out[3] = (len >> 24) & 0xff;
  out.setRange(4, out.length, payload);
  return out;
}

List<int> encodeResponseForTest(int requestId, String kind, dynamic data) {
  final frame = pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    panel: pb.Panel(
      kind: kind,
      data: jsonToProstValue(data),
      ok: true,
    ),
  );
  return encodeFrame(frame);
}

List<int> encodePageOpRequest(
  int requestId, {
  required String kind,
  int screenId = 0,
  int anchorId = 0,
  bool before = false,
  List<int> ids = const [],
  List<double> lasso = const [],
  int targetScreenId = 0,
  double dx = 0,
  double dy = 0,
  String payload = '',
  double cx = 0,
  double cy = 0,
}) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    pageOpRequest: pb.PageOpRequest(
      kind: kind,
      screenId: Int64(screenId),
      anchorId: Int64(anchorId),
      before: before,
      ids: ids.map(Int64.new).toList(),
      lasso: lasso,
      targetScreenId: Int64(targetScreenId),
      dx: dx,
      dy: dy,
      payload: payload,
      cx: cx,
      cy: cy,
    ),
  ));
}

List<int> encodeSetSettingRequest(int requestId, String key, dynamic value) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    setSetting: pb.SetSetting(
      key: key,
      value: settingValueFromDart(value),
    ),
  ));
}

List<int> encodeGetSettingsRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    getSettingsRequest: pb.GetSettingsRequest(),
  ));
}

List<int> encodeListPagesRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    listPagesRequest: pb.ListPagesRequest(),
  ));
}

List<int> encodePageThumbnailsRequest(int requestId, List<int> ids, int maxSize) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    pageThumbnailsRequest: pb.PageThumbnailsRequest(
      ids: ids.map(Int64.new).toList(),
      maxSize: maxSize,
    ),
  ));
}

List<int> encodeRequest(int requestId, String kind, Map<String, dynamic>? data) {
  final frame = pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    panel: pb.Panel(
      kind: kind,
      data: jsonToProstValue(data ?? const <String, dynamic>{}),
      ok: true,
    ),
  );
  return encodeFrame(frame);
}

PanelMessage decodePanelMessage(List<int> bytes) {
  final messages = PanelStreamDecoder().feed(bytes);
  if (messages.isEmpty) {
    throw FormatException('空 Panel protobuf');
  }
  return messages.first;
}

class PanelStreamDecoder {
  final List<int> _buffer = [];

  List<PanelMessage> feed(List<int> bytes) {
    _buffer.addAll(bytes);
    final out = <PanelMessage>[];
    while (_buffer.length >= 4) {
      final len = _buffer[0] |
          (_buffer[1] << 8) |
          (_buffer[2] << 16) |
          (_buffer[3] << 24);
      if (len < 0 || len > 8 * 1024 * 1024) {
        throw StateError('protobuf 帧长度非法: $len');
      }
      if (_buffer.length < 4 + len) {
        break;
      }
      final payload = Uint8List.fromList(_buffer.sublist(4, 4 + len));
      _buffer.removeRange(0, 4 + len);
      final frame = pb.Frame.fromBuffer(payload);
      if (frame.protocolVersion != 0 && frame.protocolVersion != 1) {
        throw StateError(
          '不兼容的面板协议版本: ${frame.protocolVersion}',
        );
      }
      if (frame.hasSettingsChanged()) {
        final snapshot = frame.settingsChanged.snapshot;
        final settings = snapshot.hasSettings() ? snapshot.settings : null;
        out.add(PanelMessage(
          'onSettingsChanged',
          0,
          settings == null
              ? const <String, dynamic>{}
              : appSettingsToMap(settings),
        ));
        continue;
      }
      if (frame.hasPageOpResponse()) {
        final r = frame.pageOpResponse;
        dynamic data = const <String, dynamic>{};
        switch (r.whichResult()) {
          case pb.PageOpResponse_Result.ok:
            data = {'ok': r.ok};
          case pb.PageOpResponse_Result.ids:
            data = {'ids': r.ids.values.map((v) => v.toInt()).toList()};
          case pb.PageOpResponse_Result.payload:
            data = {'payload': r.payload};
          case pb.PageOpResponse_Result.count:
            data = {'count': r.count};
          case pb.PageOpResponse_Result.notSet:
            break;
        }
        out.add(PanelMessage(r.kind, frame.requestId.toInt(), data));
        continue;
      }
      if (frame.hasPageThumbnailsResponse()) {
        out.add(PanelMessage(
          'getPageThumbnails_response',
          frame.requestId.toInt(),
          {
            'entries': [
              for (final t in frame.pageThumbnailsResponse.thumbnails)
                {'id': t.id.toInt(), 'png': Uint8List.fromList(t.png)},
            ],
          },
        ));
        continue;
      }
      if (frame.hasBubbleMessages()) {
        out.add(PanelMessage(
          'bubbleMessages',
          frame.requestId.toInt(),
          bubbleMessagesToMap(frame.bubbleMessages),
        ));
        continue;
      }
      if (frame.hasPageListResponse()) {
        out.add(PanelMessage(
          'listPages_response',
          frame.requestId.toInt(),
          {
            'data': [
              for (final p in frame.pageListResponse.pages)
                {
                  'id': p.id.toInt(),
                  'w': p.width,
                  'h': p.height,
                  'strokes': p.strokeCount,
                },
            ],
          },
        ));
        continue;
      }
      if (frame.hasSettingsSnapshotResponse()) {
        final snapshot = frame.settingsSnapshotResponse.snapshot;
        final settings = snapshot.hasSettings() ? snapshot.settings : null;
        out.add(PanelMessage(
          'getSettings_response',
          frame.requestId.toInt(),
          settings == null
              ? const <String, dynamic>{}
              : appSettingsToMap(settings),
        ));
        continue;
      }
      if (!frame.hasPanel()) {
        continue;
      }
      final panel = frame.panel;
      out.add(PanelMessage(
        panel.kind,
        frame.requestId.toInt(),
        panel.hasData() ? prostValueToJson(panel.data) : const <String, dynamic>{},
      ));
    }
    return out;
  }
}
