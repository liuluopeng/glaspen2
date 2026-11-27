// Windows 面板命名管道的 protobuf 分帧/Struct 转换。
// 线格式: 连接开头发送 magic, 之后每帧 = u32 little-endian 长度 + Frame bytes。
import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:protobuf/well_known_types/google/protobuf/struct.pb.dart' as wkt;

import 'glaspen/ipc/v1/ipc.pb.dart' as pb;

const List<int> panelMagic = [0x47, 0x50, 0x42, 0x31]; // "GPB1"

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
