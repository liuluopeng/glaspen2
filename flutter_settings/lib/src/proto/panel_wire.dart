// Windows 面板命名管道的 protobuf 分帧/Struct 转换。
// 线格式: 连接开头发送 magic, 之后每帧 = u32 little-endian 长度 + Frame bytes。
import 'dart:convert';
import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:protobuf/well_known_types/google/protobuf/struct.pb.dart' as wkt;

import 'glaspen/ipc/v1/ipc.pb.dart' as pb;

const List<int> panelMagic = [0x47, 0x50, 0x42, 0x31]; // "GPB1"

dynamic _uiSettingValue(pb.SettingValue? v) {
  if (v == null) return null;
  switch (v.whichValue()) {
    case pb.SettingValue_Value.boolValue:
      return v.boolValue;
    case pb.SettingValue_Value.intValue:
      return v.intValue.toInt();
    case pb.SettingValue_Value.doubleValue:
      return v.doubleValue;
    case pb.SettingValue_Value.stringValue:
      return v.stringValue;
    case pb.SettingValue_Value.nullValue:
      return null;
    default:
      return null;
  }
}

Map<String, dynamic>? _uiCondition(pb.UiSettingCondition? c) {
  if (c == null) return null;
  return {
    'key': c.key,
    'equals': _uiSettingValue(c.equals),
  };
}

List<Map<String, dynamic>> _uiKeyCaps(List<pb.UiKeyCap> caps) => [
      for (final c in caps)
        {
          'name': c.name,
          'label': c.label,
          'width': c.width,
          'modifier': c.modifier,
        }
    ];

Map<String, dynamic>? _uiForm(pb.UiFormMeta? f) {
  if (f == null) return null;
  final out = <String, dynamic>{
    'key': f.key,
    'section': f.section,
    'order': f.order,
    'labelId': f.labelId,
    'control': f.control,
    'layout': f.layout,
    'showLabel': f.showLabel,
    'indented': f.indented,
    'dividerBefore': f.dividerBefore,
    'advanced': f.advanced,
  };
  if (f.descriptionId.isNotEmpty) out['descriptionId'] = f.descriptionId;
  if (f.hintId.isNotEmpty) out['hintId'] = f.hintId;
  if (f.unit.isNotEmpty) out['unit'] = f.unit;
  if (f.hasRange()) {
    out['range'] = {
      'min': f.range.min,
      'max': f.range.max,
      'step': f.range.step,
    };
  }
  if (f.options.isNotEmpty) {
    out['options'] = [
      for (final o in f.options)
        {
          'value': _uiSettingValue(o.value),
          'labelId': o.labelId,
        }
    ];
  }
  if (f.visibleWhenFeature.isNotEmpty) {
    out['visibleWhenFeature'] = f.visibleWhenFeature;
  }
  final visible = _uiCondition(f.visibleWhenSetting);
  if (visible != null) out['visibleWhenSetting'] = visible;
  final enabled = _uiCondition(f.enabledWhenSetting);
  if (enabled != null) out['enabledWhenSetting'] = enabled;
  return out;
}

Map<String, dynamic> uiConfigToMap(pb.UiConfig c) {
  return {
    'platform': c.platform,
    'isWindows': c.isWindows,
    'fontScale': c.fontScale,
    'chordSeparator': c.chordSeparator,
    'chordNames': Map<String, String>.from(c.chordNames),
    'modifierWord': c.modifierWord,
    'messages': {
      for (final e in c.messages.entries)
        e.key: {'zh': e.value.zh, 'en': e.value.en}
    },
    'defaults': Map<String, String>.from(c.defaults),
    'holdHotkeys': Map<String, String>.from(c.holdHotkeys),
    'reserved': c.reserved.toList(),
    'forceBound': c.forceBound.toList(),
    'actionOrder': c.actionOrder.toList(),
    'modifierRow': {
      'left': _uiKeyCaps(c.modifierRow.left),
      'space': _uiKeyCaps(c.modifierRow.space),
      'right': _uiKeyCaps(c.modifierRow.right),
    },
    'features': Map<String, bool>.from(c.features),
    'settings': [
      for (final s in c.settings)
        {
          'key': s.key,
          'dbKey': s.dbKey,
          'kind': s.kind,
          'mac': s.mac,
          'win': s.win,
          'form': _uiForm(s.form),
        }
    ],
    'settingsSections': [
      for (final s in c.settingsSections)
        {
          'id': s.id,
          'labelId': s.labelId,
          'advanced': s.advanced,
          'order': s.order,
          'visibleWhenSetting': _uiCondition(s.visibleWhenSetting),
        }
    ],
    'settingsBlocks': [
      for (final s in c.settingsBlocks)
        {
          'id': s.id,
          'section': s.section,
          'order': s.order,
          'visibleWhenSetting': _uiCondition(s.visibleWhenSetting),
        }
    ],
    'protocolVersion': c.protocolVersion,
  };
}

Map<String, dynamic> decodeUiConfigFrame(List<int> bytes) {
  final data = decodePanelMessage(bytes).data;
  return data is Map ? Map<String, dynamic>.from(data) : const {};
}

Map<String, dynamic> decodeNasBackupFrame(List<int> bytes) {
  final data = decodePanelMessage(bytes).data;
  return data is Map ? Map<String, dynamic>.from(data) : const {};
}

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
    'shortcuts': jsonEncode(s.shortcuts),
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

List<int> encodeNavigateToPageRequest(int screenId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(0),
    navigateToPageRequest: pb.NavigateToPageRequest(screenId: Int64(screenId)),
  ));
}

List<int> encodeHotkeyCommand(String key) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(0),
    hotkeyCommand: pb.HotkeyCommand(key: key),
  ));
}

List<int> encodeOpenUrlCommand(String url) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(0),
    openUrlCommand: pb.OpenUrlCommand(url: url),
  ));
}

List<int> encodeFlushBubblesRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    flushBubblesRequest: pb.FlushBubblesRequest(),
  ));
}

List<int> encodeAppVersionRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    appVersionRequest: pb.AppVersionRequest(),
  ));
}

List<int> encodeDownloadUpdateRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    downloadUpdateRequest: pb.DownloadUpdateRequest(),
  ));
}

List<int> encodeCancelDownloadRequest() {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(0),
    cancelDownloadRequest: pb.CancelDownloadRequest(),
  ));
}

List<int> encodeApplyUpdateRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    applyUpdateRequest: pb.ApplyUpdateRequest(),
  ));
}

List<int> encodeNasBackupRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    nasBackupRequest: pb.NasBackupRequest(),
  ));
}

List<int> encodeCheckUpdateRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    checkUpdateRequest: pb.CheckUpdateRequest(),
  ));
}

List<int> encodeBackupCommandRequest(int requestId, pb.BackupCommand command) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    backupCommandRequest: pb.BackupCommandRequest(command: command),
  ));
}

List<int> encodeChatLoginRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    chatLoginRequest: pb.ChatLoginRequest(),
  ));
}

List<int> encodeExportPdfRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    exportPdfRequest: pb.ExportPdfRequest(),
  ));
}

List<int> encodeCanvasOverviewRequest(
    int requestId, int w, int h, pb.CanvasAction action) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    canvasOverviewRequest: pb.CanvasOverviewRequest(
      w: w,
      h: h,
      action: action,
    ),
  ));
}

List<int> encodeUiConfigRequest(int requestId, int panelProtocolVersion) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    uiConfigRequest: pb.UiConfigRequest(
      panelProtocolVersion: panelProtocolVersion,
    ),
  ));
}

List<int> encodeGetBubblesRequest(int requestId) {
  return encodeFrame(pb.Frame(
    protocolVersion: 1,
    requestId: Int64(requestId),
    getBubblesRequest: pb.GetBubblesRequest(),
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
      if (frame.hasDownloadProgress()) {
        final r = frame.downloadProgress;
        out.add(PanelMessage(
          'downloadUpdate_frame',
          frame.requestId.toInt(),
          {
            'received': r.received.toInt(),
            'total': r.total.toInt(),
            'done': r.done,
            'error': r.error,
            'path': r.path,
          },
        ));
        continue;
      }
      if (frame.hasApplyUpdateOutcome()) {
        final r = frame.applyUpdateOutcome;
        out.add(PanelMessage(
          'applyUpdate_response',
          frame.requestId.toInt(),
          {'ok': r.ok, 'message': r.message},
        ));
        continue;
      }
      if (frame.hasNasBackupOutcome()) {
        final r = frame.nasBackupOutcome;
        out.add(PanelMessage(
          'nasBackup_response',
          frame.requestId.toInt(),
          {
            'ok': r.ok,
            'size_bytes': r.sizeBytes.toInt(),
            'stroke_count': r.strokeCount.toInt(),
            'error': r.error,
            'hint': r.hint,
          },
        ));
        continue;
      }
      if (frame.hasUpdateInfo()) {
        final r = frame.updateInfo;
        out.add(PanelMessage(
          'checkUpdate_response',
          frame.requestId.toInt(),
          {
            'ok': r.ok,
            'current': r.current,
            'latest': r.latest,
            'hasUpdate': r.hasUpdate,
            'url': r.url,
            'notes': r.notes,
            'error': r.error,
          },
        ));
        continue;
      }
      if (frame.hasBackupOutcome()) {
        final r = frame.backupOutcome;
        out.add(PanelMessage(r.kind, frame.requestId.toInt(), {
          'ok': r.ok,
          'message': r.message,
        }));
        continue;
      }
      if (frame.hasChatLoginOutcome()) {
        final r = frame.chatLoginOutcome;
        out.add(PanelMessage(r.kind, frame.requestId.toInt(), {
          'ok': r.ok,
          'message': r.message,
        }));
        continue;
      }
      if (frame.hasCanvasOverviewResponse()) {
        final r = frame.canvasOverviewResponse;
        out.add(PanelMessage(
          'canvasOverview_response',
          frame.requestId.toInt(),
          {
            'png': Uint8List.fromList(r.png),
            'rect': r.rect,
          },
        ));
        continue;
      }
      if (frame.hasUiConfigResponse()) {
        out.add(PanelMessage(
          'uiConfig_response',
          frame.requestId.toInt(),
          uiConfigToMap(frame.uiConfigResponse.config),
        ));
        continue;
      }
      if (frame.hasAppVersionResponse()) {
        out.add(PanelMessage(
          'appVersion_response',
          frame.requestId.toInt(),
          {'version': frame.appVersionResponse.version},
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
          'bubbleMessages_response',
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
      if (frame.hasSettingsPatch()) {
        final values = <String, dynamic>{};
        frame.settingsPatch.values.forEach((k, v) {
          values[k] = _uiSettingValue(v);
        });
        out.add(PanelMessage('onSettingsChanged', frame.requestId.toInt(), values));
        continue;
      }
      if (frame.hasError()) {
        out.add(PanelMessage(
          'error',
          frame.requestId.toInt(),
          {'code': frame.error.code, 'message': frame.error.message},
        ));
        continue;
      }
    }
    return out;
  }
}
