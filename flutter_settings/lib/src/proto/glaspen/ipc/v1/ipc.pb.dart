// This is a generated file - do not edit.
//
// Generated from glaspen/ipc/v1/ipc.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:fixnum/fixnum.dart' as $fixnum;
import 'package:protobuf/protobuf.dart' as $pb;
import 'package:protobuf/well_known_types/google/protobuf/struct.pb.dart' as $0;

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

enum Frame_Body {
  settingsSnapshot,
  setSetting,
  settingsSnapshotResponse,
  ack,
  panel,
  error,
  notSet
}

/// 跨进程/跨语言边界的第一版 protobuf 契约。
///
/// 目标: Dart(Flutter 设置面板) <-> Rust(覆盖层) 的命名管道、以及后续
/// macOS/Flutter 的设置快照都走这一份 schema；不再把 JSON 文本放在线上。
///
/// 约定:
/// - 所有帧 = u32 little-endian 长度 + Frame protobuf bytes(长度不含前缀)；
/// - protocol_version 用于拒绝不兼容的旧客户端；
/// - request_id 由请求方生成, 响应原样带回;
/// - 新增消息只加新的 oneof 分支/字段号, 字段号永不复用。
class Frame extends $pb.GeneratedMessage {
  factory Frame({
    $core.int? protocolVersion,
    $fixnum.Int64? requestId,
    SettingsSnapshot? settingsSnapshot,
    SetSetting? setSetting,
    SettingsSnapshotResponse? settingsSnapshotResponse,
    Ack? ack,
    Panel? panel,
    Error? error,
  }) {
    final result = Frame._();
    if (protocolVersion != null) result.protocolVersion = protocolVersion;
    if (requestId != null) result.requestId = requestId;
    if (settingsSnapshot != null) result.settingsSnapshot = settingsSnapshot;
    if (setSetting != null) result.setSetting = setSetting;
    if (settingsSnapshotResponse != null)
      result.settingsSnapshotResponse = settingsSnapshotResponse;
    if (ack != null) result.ack = ack;
    if (panel != null) result.panel = panel;
    if (error != null) result.error = error;
    return result;
  }

  Frame._();

  factory Frame.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Frame()..mergeFromBuffer(data, registry);
  factory Frame.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Frame()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, Frame_Body> _Frame_BodyByTag = {
    10: Frame_Body.settingsSnapshot,
    11: Frame_Body.setSetting,
    12: Frame_Body.settingsSnapshotResponse,
    13: Frame_Body.ack,
    20: Frame_Body.panel,
    99: Frame_Body.error,
    0: Frame_Body.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Frame',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Frame.$_createMessage)
    ..oo(0, [10, 11, 12, 13, 20, 99])
    ..aI(1, _omitFieldNames ? '' : 'protocolVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..a<$fixnum.Int64>(
        2, _omitFieldNames ? '' : 'requestId', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aOM<SettingsSnapshot>(10, _omitFieldNames ? '' : 'settingsSnapshot',
        subBuilder: SettingsSnapshot.$_createMessage)
    ..aOM<SetSetting>(11, _omitFieldNames ? '' : 'setSetting',
        subBuilder: SetSetting.$_createMessage)
    ..aOM<SettingsSnapshotResponse>(
        12, _omitFieldNames ? '' : 'settingsSnapshotResponse',
        subBuilder: SettingsSnapshotResponse.$_createMessage)
    ..aOM<Ack>(13, _omitFieldNames ? '' : 'ack',
        subBuilder: Ack.$_createMessage)
    ..aOM<Panel>(20, _omitFieldNames ? '' : 'panel',
        subBuilder: Panel.$_createMessage)
    ..aOM<Error>(99, _omitFieldNames ? '' : 'error',
        subBuilder: Error.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Frame clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Frame copyWith(void Function(Frame) updates) =>
      super.copyWith((message) => updates(message as Frame)) as Frame;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Frame() / Frame.new instead')
  static Frame create() => Frame._();
  static $pb.GeneratedMessage $_createMessage() => Frame._();
  @$core.override
  Frame createEmptyInstance() => Frame._();
  @$core.pragma('dart2js:noInline')
  static Frame getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Frame>(Frame.$_createMessage);
  static Frame? _defaultInstance;

  @$pb.TagNumber(10)
  @$pb.TagNumber(11)
  @$pb.TagNumber(12)
  @$pb.TagNumber(13)
  @$pb.TagNumber(20)
  @$pb.TagNumber(99)
  Frame_Body whichBody() => _Frame_BodyByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(10)
  @$pb.TagNumber(11)
  @$pb.TagNumber(12)
  @$pb.TagNumber(13)
  @$pb.TagNumber(20)
  @$pb.TagNumber(99)
  void clearBody() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  $core.int get protocolVersion => $_getIZ(0);
  @$pb.TagNumber(1)
  set protocolVersion($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasProtocolVersion() => $_has(0);
  @$pb.TagNumber(1)
  void clearProtocolVersion() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get requestId => $_getI64(1);
  @$pb.TagNumber(2)
  set requestId($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasRequestId() => $_has(1);
  @$pb.TagNumber(2)
  void clearRequestId() => $_clearField(2);

  @$pb.TagNumber(10)
  SettingsSnapshot get settingsSnapshot => $_getN(2);
  @$pb.TagNumber(10)
  set settingsSnapshot(SettingsSnapshot value) => $_setField(10, value);
  @$pb.TagNumber(10)
  $core.bool hasSettingsSnapshot() => $_has(2);
  @$pb.TagNumber(10)
  void clearSettingsSnapshot() => $_clearField(10);
  @$pb.TagNumber(10)
  SettingsSnapshot ensureSettingsSnapshot() => $_ensure(2);

  @$pb.TagNumber(11)
  SetSetting get setSetting => $_getN(3);
  @$pb.TagNumber(11)
  set setSetting(SetSetting value) => $_setField(11, value);
  @$pb.TagNumber(11)
  $core.bool hasSetSetting() => $_has(3);
  @$pb.TagNumber(11)
  void clearSetSetting() => $_clearField(11);
  @$pb.TagNumber(11)
  SetSetting ensureSetSetting() => $_ensure(3);

  @$pb.TagNumber(12)
  SettingsSnapshotResponse get settingsSnapshotResponse => $_getN(4);
  @$pb.TagNumber(12)
  set settingsSnapshotResponse(SettingsSnapshotResponse value) =>
      $_setField(12, value);
  @$pb.TagNumber(12)
  $core.bool hasSettingsSnapshotResponse() => $_has(4);
  @$pb.TagNumber(12)
  void clearSettingsSnapshotResponse() => $_clearField(12);
  @$pb.TagNumber(12)
  SettingsSnapshotResponse ensureSettingsSnapshotResponse() => $_ensure(4);

  @$pb.TagNumber(13)
  Ack get ack => $_getN(5);
  @$pb.TagNumber(13)
  set ack(Ack value) => $_setField(13, value);
  @$pb.TagNumber(13)
  $core.bool hasAck() => $_has(5);
  @$pb.TagNumber(13)
  void clearAck() => $_clearField(13);
  @$pb.TagNumber(13)
  Ack ensureAck() => $_ensure(5);

  @$pb.TagNumber(20)
  Panel get panel => $_getN(6);
  @$pb.TagNumber(20)
  set panel(Panel value) => $_setField(20, value);
  @$pb.TagNumber(20)
  $core.bool hasPanel() => $_has(6);
  @$pb.TagNumber(20)
  void clearPanel() => $_clearField(20);
  @$pb.TagNumber(20)
  Panel ensurePanel() => $_ensure(6);

  @$pb.TagNumber(99)
  Error get error => $_getN(7);
  @$pb.TagNumber(99)
  set error(Error value) => $_setField(99, value);
  @$pb.TagNumber(99)
  $core.bool hasError() => $_has(7);
  @$pb.TagNumber(99)
  void clearError() => $_clearField(99);
  @$pb.TagNumber(99)
  Error ensureError() => $_ensure(7);
}

enum SettingValue_Value {
  boolValue,
  intValue,
  doubleValue,
  stringValue,
  structValue,
  listValue,
  bytesValue,
  nullValue,
  notSet
}

/// 设置值: 面板会传 bool/int/double/string/嵌套对象/数组。
/// Struct 覆盖动态 JSON 兼容值; 后续稳定字段可以逐步改成 typed 子消息。
class SettingValue extends $pb.GeneratedMessage {
  factory SettingValue({
    $core.bool? boolValue,
    $fixnum.Int64? intValue,
    $core.double? doubleValue,
    $core.String? stringValue,
    $0.Struct? structValue,
    ListValue? listValue,
    $core.List<$core.int>? bytesValue,
    $core.bool? nullValue,
  }) {
    final result = SettingValue._();
    if (boolValue != null) result.boolValue = boolValue;
    if (intValue != null) result.intValue = intValue;
    if (doubleValue != null) result.doubleValue = doubleValue;
    if (stringValue != null) result.stringValue = stringValue;
    if (structValue != null) result.structValue = structValue;
    if (listValue != null) result.listValue = listValue;
    if (bytesValue != null) result.bytesValue = bytesValue;
    if (nullValue != null) result.nullValue = nullValue;
    return result;
  }

  SettingValue._();

  factory SettingValue.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingValue()..mergeFromBuffer(data, registry);
  factory SettingValue.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingValue()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, SettingValue_Value>
      _SettingValue_ValueByTag = {
    1: SettingValue_Value.boolValue,
    2: SettingValue_Value.intValue,
    3: SettingValue_Value.doubleValue,
    4: SettingValue_Value.stringValue,
    5: SettingValue_Value.structValue,
    6: SettingValue_Value.listValue,
    7: SettingValue_Value.bytesValue,
    8: SettingValue_Value.nullValue,
    0: SettingValue_Value.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingValue',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingValue.$_createMessage)
    ..oo(0, [1, 2, 3, 4, 5, 6, 7, 8])
    ..aOB(1, _omitFieldNames ? '' : 'boolValue')
    ..aInt64(2, _omitFieldNames ? '' : 'intValue')
    ..aD(3, _omitFieldNames ? '' : 'doubleValue')
    ..aOS(4, _omitFieldNames ? '' : 'stringValue')
    ..aOM<$0.Struct>(5, _omitFieldNames ? '' : 'structValue',
        subBuilder: $0.Struct.$_createMessage)
    ..aOM<ListValue>(6, _omitFieldNames ? '' : 'listValue',
        subBuilder: ListValue.$_createMessage)
    ..a<$core.List<$core.int>>(
        7, _omitFieldNames ? '' : 'bytesValue', $pb.PbFieldType.OY)
    ..aOB(8, _omitFieldNames ? '' : 'nullValue')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingValue clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingValue copyWith(void Function(SettingValue) updates) =>
      super.copyWith((message) => updates(message as SettingValue))
          as SettingValue;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use SettingValue() / SettingValue.new instead')
  static SettingValue create() => SettingValue._();
  static $pb.GeneratedMessage $_createMessage() => SettingValue._();
  @$core.override
  SettingValue createEmptyInstance() => SettingValue._();
  @$core.pragma('dart2js:noInline')
  static SettingValue getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<SettingValue>(
          SettingValue.$_createMessage);
  static SettingValue? _defaultInstance;

  @$pb.TagNumber(1)
  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  SettingValue_Value whichValue() => _SettingValue_ValueByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(1)
  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  void clearValue() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  $core.bool get boolValue => $_getBF(0);
  @$pb.TagNumber(1)
  set boolValue($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasBoolValue() => $_has(0);
  @$pb.TagNumber(1)
  void clearBoolValue() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get intValue => $_getI64(1);
  @$pb.TagNumber(2)
  set intValue($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasIntValue() => $_has(1);
  @$pb.TagNumber(2)
  void clearIntValue() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.double get doubleValue => $_getN(2);
  @$pb.TagNumber(3)
  set doubleValue($core.double value) => $_setDouble(2, value);
  @$pb.TagNumber(3)
  $core.bool hasDoubleValue() => $_has(2);
  @$pb.TagNumber(3)
  void clearDoubleValue() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get stringValue => $_getSZ(3);
  @$pb.TagNumber(4)
  set stringValue($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasStringValue() => $_has(3);
  @$pb.TagNumber(4)
  void clearStringValue() => $_clearField(4);

  @$pb.TagNumber(5)
  $0.Struct get structValue => $_getN(4);
  @$pb.TagNumber(5)
  set structValue($0.Struct value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasStructValue() => $_has(4);
  @$pb.TagNumber(5)
  void clearStructValue() => $_clearField(5);
  @$pb.TagNumber(5)
  $0.Struct ensureStructValue() => $_ensure(4);

  @$pb.TagNumber(6)
  ListValue get listValue => $_getN(5);
  @$pb.TagNumber(6)
  set listValue(ListValue value) => $_setField(6, value);
  @$pb.TagNumber(6)
  $core.bool hasListValue() => $_has(5);
  @$pb.TagNumber(6)
  void clearListValue() => $_clearField(6);
  @$pb.TagNumber(6)
  ListValue ensureListValue() => $_ensure(5);

  @$pb.TagNumber(7)
  $core.List<$core.int> get bytesValue => $_getN(6);
  @$pb.TagNumber(7)
  set bytesValue($core.List<$core.int> value) => $_setBytes(6, value);
  @$pb.TagNumber(7)
  $core.bool hasBytesValue() => $_has(6);
  @$pb.TagNumber(7)
  void clearBytesValue() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.bool get nullValue => $_getBF(7);
  @$pb.TagNumber(8)
  set nullValue($core.bool value) => $_setBool(7, value);
  @$pb.TagNumber(8)
  $core.bool hasNullValue() => $_has(7);
  @$pb.TagNumber(8)
  void clearNullValue() => $_clearField(8);
}

class ListValue extends $pb.GeneratedMessage {
  factory ListValue({
    $core.Iterable<SettingValue>? values,
  }) {
    final result = ListValue._();
    if (values != null) result.values.addAll(values);
    return result;
  }

  ListValue._();

  factory ListValue.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListValue()..mergeFromBuffer(data, registry);
  factory ListValue.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListValue()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListValue',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ListValue.$_createMessage)
    ..pPM<SettingValue>(1, _omitFieldNames ? '' : 'values',
        subBuilder: SettingValue.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListValue clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListValue copyWith(void Function(ListValue) updates) =>
      super.copyWith((message) => updates(message as ListValue)) as ListValue;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ListValue() / ListValue.new instead')
  static ListValue create() => ListValue._();
  static $pb.GeneratedMessage $_createMessage() => ListValue._();
  @$core.override
  ListValue createEmptyInstance() => ListValue._();
  @$core.pragma('dart2js:noInline')
  static ListValue getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ListValue>(ListValue.$_createMessage);
  static ListValue? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<SettingValue> get values => $_getList(0);
}

class SettingsSnapshot extends $pb.GeneratedMessage {
  factory SettingsSnapshot({
    $core.Iterable<$core.MapEntry<$core.String, SettingValue>>? values,
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? metadata,
  }) {
    final result = SettingsSnapshot._();
    if (values != null) result.values.addEntries(values);
    if (metadata != null) result.metadata.addEntries(metadata);
    return result;
  }

  SettingsSnapshot._();

  factory SettingsSnapshot.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsSnapshot()..mergeFromBuffer(data, registry);
  factory SettingsSnapshot.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsSnapshot()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingsSnapshot',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingsSnapshot.$_createMessage)
    ..m<$core.String, SettingValue>(1, _omitFieldNames ? '' : 'values',
        entryClassName: 'SettingsSnapshot.ValuesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OM,
        valueCreator: SettingValue.$_createMessage,
        valueDefaultOrMaker: SettingValue.getDefault,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..m<$core.String, $core.String>(2, _omitFieldNames ? '' : 'metadata',
        entryClassName: 'SettingsSnapshot.MetadataEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsSnapshot clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsSnapshot copyWith(void Function(SettingsSnapshot) updates) =>
      super.copyWith((message) => updates(message as SettingsSnapshot))
          as SettingsSnapshot;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use SettingsSnapshot() / SettingsSnapshot.new instead')
  static SettingsSnapshot create() => SettingsSnapshot._();
  static $pb.GeneratedMessage $_createMessage() => SettingsSnapshot._();
  @$core.override
  SettingsSnapshot createEmptyInstance() => SettingsSnapshot._();
  @$core.pragma('dart2js:noInline')
  static SettingsSnapshot getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<SettingsSnapshot>(
          SettingsSnapshot.$_createMessage);
  static SettingsSnapshot? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbMap<$core.String, SettingValue> get values => $_getMap(0);

  @$pb.TagNumber(2)
  $pb.PbMap<$core.String, $core.String> get metadata => $_getMap(1);
}

class SettingsSnapshotResponse extends $pb.GeneratedMessage {
  factory SettingsSnapshotResponse({
    SettingsSnapshot? snapshot,
  }) {
    final result = SettingsSnapshotResponse._();
    if (snapshot != null) result.snapshot = snapshot;
    return result;
  }

  SettingsSnapshotResponse._();

  factory SettingsSnapshotResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsSnapshotResponse()..mergeFromBuffer(data, registry);
  factory SettingsSnapshotResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsSnapshotResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingsSnapshotResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingsSnapshotResponse.$_createMessage)
    ..aOM<SettingsSnapshot>(1, _omitFieldNames ? '' : 'snapshot',
        subBuilder: SettingsSnapshot.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsSnapshotResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsSnapshotResponse copyWith(
          void Function(SettingsSnapshotResponse) updates) =>
      super.copyWith((message) => updates(message as SettingsSnapshotResponse))
          as SettingsSnapshotResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use SettingsSnapshotResponse() / SettingsSnapshotResponse.new instead')
  static SettingsSnapshotResponse create() => SettingsSnapshotResponse._();
  static $pb.GeneratedMessage $_createMessage() => SettingsSnapshotResponse._();
  @$core.override
  SettingsSnapshotResponse createEmptyInstance() =>
      SettingsSnapshotResponse._();
  @$core.pragma('dart2js:noInline')
  static SettingsSnapshotResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<SettingsSnapshotResponse>(
          SettingsSnapshotResponse.$_createMessage);
  static SettingsSnapshotResponse? _defaultInstance;

  @$pb.TagNumber(1)
  SettingsSnapshot get snapshot => $_getN(0);
  @$pb.TagNumber(1)
  set snapshot(SettingsSnapshot value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasSnapshot() => $_has(0);
  @$pb.TagNumber(1)
  void clearSnapshot() => $_clearField(1);
  @$pb.TagNumber(1)
  SettingsSnapshot ensureSnapshot() => $_ensure(0);
}

class SetSetting extends $pb.GeneratedMessage {
  factory SetSetting({
    $core.String? key,
    SettingValue? value,
  }) {
    final result = SetSetting._();
    if (key != null) result.key = key;
    if (value != null) result.value = value;
    return result;
  }

  SetSetting._();

  factory SetSetting.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SetSetting()..mergeFromBuffer(data, registry);
  factory SetSetting.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SetSetting()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SetSetting',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SetSetting.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'key')
    ..aOM<SettingValue>(2, _omitFieldNames ? '' : 'value',
        subBuilder: SettingValue.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SetSetting clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SetSetting copyWith(void Function(SetSetting) updates) =>
      super.copyWith((message) => updates(message as SetSetting)) as SetSetting;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use SetSetting() / SetSetting.new instead')
  static SetSetting create() => SetSetting._();
  static $pb.GeneratedMessage $_createMessage() => SetSetting._();
  @$core.override
  SetSetting createEmptyInstance() => SetSetting._();
  @$core.pragma('dart2js:noInline')
  static SetSetting getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<SetSetting>(SetSetting.$_createMessage);
  static SetSetting? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get key => $_getSZ(0);
  @$pb.TagNumber(1)
  set key($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearKey() => $_clearField(1);

  @$pb.TagNumber(2)
  SettingValue get value => $_getN(1);
  @$pb.TagNumber(2)
  set value(SettingValue value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasValue() => $_has(1);
  @$pb.TagNumber(2)
  void clearValue() => $_clearField(2);
  @$pb.TagNumber(2)
  SettingValue ensureValue() => $_ensure(1);
}

class Ack extends $pb.GeneratedMessage {
  factory Ack({
    $core.bool? ok,
  }) {
    final result = Ack._();
    if (ok != null) result.ok = ok;
    return result;
  }

  Ack._();

  factory Ack.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Ack()..mergeFromBuffer(data, registry);
  factory Ack.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Ack()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Ack',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Ack.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Ack clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Ack copyWith(void Function(Ack) updates) =>
      super.copyWith((message) => updates(message as Ack)) as Ack;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Ack() / Ack.new instead')
  static Ack create() => Ack._();
  static $pb.GeneratedMessage $_createMessage() => Ack._();
  @$core.override
  Ack createEmptyInstance() => Ack._();
  @$core.pragma('dart2js:noInline')
  static Ack getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Ack>(Ack.$_createMessage);
  static Ack? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);
}

class Error extends $pb.GeneratedMessage {
  factory Error({
    $core.String? code,
    $core.String? message,
  }) {
    final result = Error._();
    if (code != null) result.code = code;
    if (message != null) result.message = message;
    return result;
  }

  Error._();

  factory Error.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Error()..mergeFromBuffer(data, registry);
  factory Error.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Error()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Error',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Error.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'code')
    ..aOS(2, _omitFieldNames ? '' : 'message')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Error clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Error copyWith(void Function(Error) updates) =>
      super.copyWith((message) => updates(message as Error)) as Error;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Error() / Error.new instead')
  static Error create() => Error._();
  static $pb.GeneratedMessage $_createMessage() => Error._();
  @$core.override
  Error createEmptyInstance() => Error._();
  @$core.pragma('dart2js:noInline')
  static Error getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Error>(Error.$_createMessage);
  static Error? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get code => $_getSZ(0);
  @$pb.TagNumber(1)
  set code($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearCode() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get message => $_getSZ(1);
  @$pb.TagNumber(2)
  set message($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMessage() => $_has(1);
  @$pb.TagNumber(2)
  void clearMessage() => $_clearField(2);
}

/// Windows 面板管道的过渡信封: kind = 原 JSON 的 type 字符串,
/// data = 原 data 对象(Struct 二进制, 不再是 JSON 文本)。
/// 后续按消息逐步改成 typed 子消息, 这个 message 作为兼容入口保留。
class Panel extends $pb.GeneratedMessage {
  factory Panel({
    $core.String? kind,
    $0.Value? data,
    $core.bool? ok,
  }) {
    final result = Panel._();
    if (kind != null) result.kind = kind;
    if (data != null) result.data = data;
    if (ok != null) result.ok = ok;
    return result;
  }

  Panel._();

  factory Panel.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Panel()..mergeFromBuffer(data, registry);
  factory Panel.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Panel()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Panel',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Panel.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'kind')
    ..aOM<$0.Value>(2, _omitFieldNames ? '' : 'data',
        subBuilder: $0.Value.$_createMessage)
    ..aOB(3, _omitFieldNames ? '' : 'ok')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Panel clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Panel copyWith(void Function(Panel) updates) =>
      super.copyWith((message) => updates(message as Panel)) as Panel;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Panel() / Panel.new instead')
  static Panel create() => Panel._();
  static $pb.GeneratedMessage $_createMessage() => Panel._();
  @$core.override
  Panel createEmptyInstance() => Panel._();
  @$core.pragma('dart2js:noInline')
  static Panel getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Panel>(Panel.$_createMessage);
  static Panel? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get kind => $_getSZ(0);
  @$pb.TagNumber(1)
  set kind($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKind() => $_has(0);
  @$pb.TagNumber(1)
  void clearKind() => $_clearField(1);

  @$pb.TagNumber(2)
  $0.Value get data => $_getN(1);
  @$pb.TagNumber(2)
  set data($0.Value value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasData() => $_has(1);
  @$pb.TagNumber(2)
  void clearData() => $_clearField(2);
  @$pb.TagNumber(2)
  $0.Value ensureData() => $_ensure(1);

  @$pb.TagNumber(3)
  $core.bool get ok => $_getBF(2);
  @$pb.TagNumber(3)
  set ok($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasOk() => $_has(2);
  @$pb.TagNumber(3)
  void clearOk() => $_clearField(3);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
