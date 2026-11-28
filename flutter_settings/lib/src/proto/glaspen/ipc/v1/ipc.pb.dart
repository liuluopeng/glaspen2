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
  settingsChanged,
  pageListResponse,
  pageThumbnailsResponse,
  bubbleMessages,
  pageOpResponse,
  panel,
  getSettingsRequest,
  listPagesRequest,
  pageThumbnailsRequest,
  pageOpRequest,
  getBubblesRequest,
  navigateToPageRequest,
  hotkeyCommand,
  openUrlCommand,
  flushBubblesRequest,
  appVersionRequest,
  appVersionResponse,
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
    SettingsChanged? settingsChanged,
    PageListResponse? pageListResponse,
    PageThumbnailsResponse? pageThumbnailsResponse,
    BubbleMessages? bubbleMessages,
    PageOpResponse? pageOpResponse,
    Panel? panel,
    GetSettingsRequest? getSettingsRequest,
    ListPagesRequest? listPagesRequest,
    PageThumbnailsRequest? pageThumbnailsRequest,
    PageOpRequest? pageOpRequest,
    GetBubblesRequest? getBubblesRequest,
    NavigateToPageRequest? navigateToPageRequest,
    HotkeyCommand? hotkeyCommand,
    OpenUrlCommand? openUrlCommand,
    FlushBubblesRequest? flushBubblesRequest,
    AppVersionRequest? appVersionRequest,
    AppVersionResponse? appVersionResponse,
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
    if (settingsChanged != null) result.settingsChanged = settingsChanged;
    if (pageListResponse != null) result.pageListResponse = pageListResponse;
    if (pageThumbnailsResponse != null)
      result.pageThumbnailsResponse = pageThumbnailsResponse;
    if (bubbleMessages != null) result.bubbleMessages = bubbleMessages;
    if (pageOpResponse != null) result.pageOpResponse = pageOpResponse;
    if (panel != null) result.panel = panel;
    if (getSettingsRequest != null)
      result.getSettingsRequest = getSettingsRequest;
    if (listPagesRequest != null) result.listPagesRequest = listPagesRequest;
    if (pageThumbnailsRequest != null)
      result.pageThumbnailsRequest = pageThumbnailsRequest;
    if (pageOpRequest != null) result.pageOpRequest = pageOpRequest;
    if (getBubblesRequest != null) result.getBubblesRequest = getBubblesRequest;
    if (navigateToPageRequest != null)
      result.navigateToPageRequest = navigateToPageRequest;
    if (hotkeyCommand != null) result.hotkeyCommand = hotkeyCommand;
    if (openUrlCommand != null) result.openUrlCommand = openUrlCommand;
    if (flushBubblesRequest != null)
      result.flushBubblesRequest = flushBubblesRequest;
    if (appVersionRequest != null) result.appVersionRequest = appVersionRequest;
    if (appVersionResponse != null)
      result.appVersionResponse = appVersionResponse;
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
    14: Frame_Body.settingsChanged,
    15: Frame_Body.pageListResponse,
    16: Frame_Body.pageThumbnailsResponse,
    17: Frame_Body.bubbleMessages,
    18: Frame_Body.pageOpResponse,
    20: Frame_Body.panel,
    30: Frame_Body.getSettingsRequest,
    31: Frame_Body.listPagesRequest,
    32: Frame_Body.pageThumbnailsRequest,
    33: Frame_Body.pageOpRequest,
    34: Frame_Body.getBubblesRequest,
    40: Frame_Body.navigateToPageRequest,
    41: Frame_Body.hotkeyCommand,
    42: Frame_Body.openUrlCommand,
    43: Frame_Body.flushBubblesRequest,
    44: Frame_Body.appVersionRequest,
    45: Frame_Body.appVersionResponse,
    99: Frame_Body.error,
    0: Frame_Body.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Frame',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Frame.$_createMessage)
    ..oo(0, [
      10,
      11,
      12,
      13,
      14,
      15,
      16,
      17,
      18,
      20,
      30,
      31,
      32,
      33,
      34,
      40,
      41,
      42,
      43,
      44,
      45,
      99
    ])
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
    ..aOM<SettingsChanged>(14, _omitFieldNames ? '' : 'settingsChanged',
        subBuilder: SettingsChanged.$_createMessage)
    ..aOM<PageListResponse>(15, _omitFieldNames ? '' : 'pageListResponse',
        subBuilder: PageListResponse.$_createMessage)
    ..aOM<PageThumbnailsResponse>(
        16, _omitFieldNames ? '' : 'pageThumbnailsResponse',
        subBuilder: PageThumbnailsResponse.$_createMessage)
    ..aOM<BubbleMessages>(17, _omitFieldNames ? '' : 'bubbleMessages',
        subBuilder: BubbleMessages.$_createMessage)
    ..aOM<PageOpResponse>(18, _omitFieldNames ? '' : 'pageOpResponse',
        subBuilder: PageOpResponse.$_createMessage)
    ..aOM<Panel>(20, _omitFieldNames ? '' : 'panel',
        subBuilder: Panel.$_createMessage)
    ..aOM<GetSettingsRequest>(30, _omitFieldNames ? '' : 'getSettingsRequest',
        subBuilder: GetSettingsRequest.$_createMessage)
    ..aOM<ListPagesRequest>(31, _omitFieldNames ? '' : 'listPagesRequest',
        subBuilder: ListPagesRequest.$_createMessage)
    ..aOM<PageThumbnailsRequest>(
        32, _omitFieldNames ? '' : 'pageThumbnailsRequest',
        subBuilder: PageThumbnailsRequest.$_createMessage)
    ..aOM<PageOpRequest>(33, _omitFieldNames ? '' : 'pageOpRequest',
        subBuilder: PageOpRequest.$_createMessage)
    ..aOM<GetBubblesRequest>(34, _omitFieldNames ? '' : 'getBubblesRequest',
        subBuilder: GetBubblesRequest.$_createMessage)
    ..aOM<NavigateToPageRequest>(
        40, _omitFieldNames ? '' : 'navigateToPageRequest',
        subBuilder: NavigateToPageRequest.$_createMessage)
    ..aOM<HotkeyCommand>(41, _omitFieldNames ? '' : 'hotkeyCommand',
        subBuilder: HotkeyCommand.$_createMessage)
    ..aOM<OpenUrlCommand>(42, _omitFieldNames ? '' : 'openUrlCommand',
        subBuilder: OpenUrlCommand.$_createMessage)
    ..aOM<FlushBubblesRequest>(43, _omitFieldNames ? '' : 'flushBubblesRequest',
        subBuilder: FlushBubblesRequest.$_createMessage)
    ..aOM<AppVersionRequest>(44, _omitFieldNames ? '' : 'appVersionRequest',
        subBuilder: AppVersionRequest.$_createMessage)
    ..aOM<AppVersionResponse>(45, _omitFieldNames ? '' : 'appVersionResponse',
        subBuilder: AppVersionResponse.$_createMessage)
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
  @$pb.TagNumber(14)
  @$pb.TagNumber(15)
  @$pb.TagNumber(16)
  @$pb.TagNumber(17)
  @$pb.TagNumber(18)
  @$pb.TagNumber(20)
  @$pb.TagNumber(30)
  @$pb.TagNumber(31)
  @$pb.TagNumber(32)
  @$pb.TagNumber(33)
  @$pb.TagNumber(34)
  @$pb.TagNumber(40)
  @$pb.TagNumber(41)
  @$pb.TagNumber(42)
  @$pb.TagNumber(43)
  @$pb.TagNumber(44)
  @$pb.TagNumber(45)
  @$pb.TagNumber(99)
  Frame_Body whichBody() => _Frame_BodyByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(10)
  @$pb.TagNumber(11)
  @$pb.TagNumber(12)
  @$pb.TagNumber(13)
  @$pb.TagNumber(14)
  @$pb.TagNumber(15)
  @$pb.TagNumber(16)
  @$pb.TagNumber(17)
  @$pb.TagNumber(18)
  @$pb.TagNumber(20)
  @$pb.TagNumber(30)
  @$pb.TagNumber(31)
  @$pb.TagNumber(32)
  @$pb.TagNumber(33)
  @$pb.TagNumber(34)
  @$pb.TagNumber(40)
  @$pb.TagNumber(41)
  @$pb.TagNumber(42)
  @$pb.TagNumber(43)
  @$pb.TagNumber(44)
  @$pb.TagNumber(45)
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

  @$pb.TagNumber(14)
  SettingsChanged get settingsChanged => $_getN(6);
  @$pb.TagNumber(14)
  set settingsChanged(SettingsChanged value) => $_setField(14, value);
  @$pb.TagNumber(14)
  $core.bool hasSettingsChanged() => $_has(6);
  @$pb.TagNumber(14)
  void clearSettingsChanged() => $_clearField(14);
  @$pb.TagNumber(14)
  SettingsChanged ensureSettingsChanged() => $_ensure(6);

  @$pb.TagNumber(15)
  PageListResponse get pageListResponse => $_getN(7);
  @$pb.TagNumber(15)
  set pageListResponse(PageListResponse value) => $_setField(15, value);
  @$pb.TagNumber(15)
  $core.bool hasPageListResponse() => $_has(7);
  @$pb.TagNumber(15)
  void clearPageListResponse() => $_clearField(15);
  @$pb.TagNumber(15)
  PageListResponse ensurePageListResponse() => $_ensure(7);

  @$pb.TagNumber(16)
  PageThumbnailsResponse get pageThumbnailsResponse => $_getN(8);
  @$pb.TagNumber(16)
  set pageThumbnailsResponse(PageThumbnailsResponse value) =>
      $_setField(16, value);
  @$pb.TagNumber(16)
  $core.bool hasPageThumbnailsResponse() => $_has(8);
  @$pb.TagNumber(16)
  void clearPageThumbnailsResponse() => $_clearField(16);
  @$pb.TagNumber(16)
  PageThumbnailsResponse ensurePageThumbnailsResponse() => $_ensure(8);

  @$pb.TagNumber(17)
  BubbleMessages get bubbleMessages => $_getN(9);
  @$pb.TagNumber(17)
  set bubbleMessages(BubbleMessages value) => $_setField(17, value);
  @$pb.TagNumber(17)
  $core.bool hasBubbleMessages() => $_has(9);
  @$pb.TagNumber(17)
  void clearBubbleMessages() => $_clearField(17);
  @$pb.TagNumber(17)
  BubbleMessages ensureBubbleMessages() => $_ensure(9);

  @$pb.TagNumber(18)
  PageOpResponse get pageOpResponse => $_getN(10);
  @$pb.TagNumber(18)
  set pageOpResponse(PageOpResponse value) => $_setField(18, value);
  @$pb.TagNumber(18)
  $core.bool hasPageOpResponse() => $_has(10);
  @$pb.TagNumber(18)
  void clearPageOpResponse() => $_clearField(18);
  @$pb.TagNumber(18)
  PageOpResponse ensurePageOpResponse() => $_ensure(10);

  @$pb.TagNumber(20)
  Panel get panel => $_getN(11);
  @$pb.TagNumber(20)
  set panel(Panel value) => $_setField(20, value);
  @$pb.TagNumber(20)
  $core.bool hasPanel() => $_has(11);
  @$pb.TagNumber(20)
  void clearPanel() => $_clearField(20);
  @$pb.TagNumber(20)
  Panel ensurePanel() => $_ensure(11);

  @$pb.TagNumber(30)
  GetSettingsRequest get getSettingsRequest => $_getN(12);
  @$pb.TagNumber(30)
  set getSettingsRequest(GetSettingsRequest value) => $_setField(30, value);
  @$pb.TagNumber(30)
  $core.bool hasGetSettingsRequest() => $_has(12);
  @$pb.TagNumber(30)
  void clearGetSettingsRequest() => $_clearField(30);
  @$pb.TagNumber(30)
  GetSettingsRequest ensureGetSettingsRequest() => $_ensure(12);

  @$pb.TagNumber(31)
  ListPagesRequest get listPagesRequest => $_getN(13);
  @$pb.TagNumber(31)
  set listPagesRequest(ListPagesRequest value) => $_setField(31, value);
  @$pb.TagNumber(31)
  $core.bool hasListPagesRequest() => $_has(13);
  @$pb.TagNumber(31)
  void clearListPagesRequest() => $_clearField(31);
  @$pb.TagNumber(31)
  ListPagesRequest ensureListPagesRequest() => $_ensure(13);

  @$pb.TagNumber(32)
  PageThumbnailsRequest get pageThumbnailsRequest => $_getN(14);
  @$pb.TagNumber(32)
  set pageThumbnailsRequest(PageThumbnailsRequest value) =>
      $_setField(32, value);
  @$pb.TagNumber(32)
  $core.bool hasPageThumbnailsRequest() => $_has(14);
  @$pb.TagNumber(32)
  void clearPageThumbnailsRequest() => $_clearField(32);
  @$pb.TagNumber(32)
  PageThumbnailsRequest ensurePageThumbnailsRequest() => $_ensure(14);

  @$pb.TagNumber(33)
  PageOpRequest get pageOpRequest => $_getN(15);
  @$pb.TagNumber(33)
  set pageOpRequest(PageOpRequest value) => $_setField(33, value);
  @$pb.TagNumber(33)
  $core.bool hasPageOpRequest() => $_has(15);
  @$pb.TagNumber(33)
  void clearPageOpRequest() => $_clearField(33);
  @$pb.TagNumber(33)
  PageOpRequest ensurePageOpRequest() => $_ensure(15);

  @$pb.TagNumber(34)
  GetBubblesRequest get getBubblesRequest => $_getN(16);
  @$pb.TagNumber(34)
  set getBubblesRequest(GetBubblesRequest value) => $_setField(34, value);
  @$pb.TagNumber(34)
  $core.bool hasGetBubblesRequest() => $_has(16);
  @$pb.TagNumber(34)
  void clearGetBubblesRequest() => $_clearField(34);
  @$pb.TagNumber(34)
  GetBubblesRequest ensureGetBubblesRequest() => $_ensure(16);

  @$pb.TagNumber(40)
  NavigateToPageRequest get navigateToPageRequest => $_getN(17);
  @$pb.TagNumber(40)
  set navigateToPageRequest(NavigateToPageRequest value) =>
      $_setField(40, value);
  @$pb.TagNumber(40)
  $core.bool hasNavigateToPageRequest() => $_has(17);
  @$pb.TagNumber(40)
  void clearNavigateToPageRequest() => $_clearField(40);
  @$pb.TagNumber(40)
  NavigateToPageRequest ensureNavigateToPageRequest() => $_ensure(17);

  @$pb.TagNumber(41)
  HotkeyCommand get hotkeyCommand => $_getN(18);
  @$pb.TagNumber(41)
  set hotkeyCommand(HotkeyCommand value) => $_setField(41, value);
  @$pb.TagNumber(41)
  $core.bool hasHotkeyCommand() => $_has(18);
  @$pb.TagNumber(41)
  void clearHotkeyCommand() => $_clearField(41);
  @$pb.TagNumber(41)
  HotkeyCommand ensureHotkeyCommand() => $_ensure(18);

  @$pb.TagNumber(42)
  OpenUrlCommand get openUrlCommand => $_getN(19);
  @$pb.TagNumber(42)
  set openUrlCommand(OpenUrlCommand value) => $_setField(42, value);
  @$pb.TagNumber(42)
  $core.bool hasOpenUrlCommand() => $_has(19);
  @$pb.TagNumber(42)
  void clearOpenUrlCommand() => $_clearField(42);
  @$pb.TagNumber(42)
  OpenUrlCommand ensureOpenUrlCommand() => $_ensure(19);

  @$pb.TagNumber(43)
  FlushBubblesRequest get flushBubblesRequest => $_getN(20);
  @$pb.TagNumber(43)
  set flushBubblesRequest(FlushBubblesRequest value) => $_setField(43, value);
  @$pb.TagNumber(43)
  $core.bool hasFlushBubblesRequest() => $_has(20);
  @$pb.TagNumber(43)
  void clearFlushBubblesRequest() => $_clearField(43);
  @$pb.TagNumber(43)
  FlushBubblesRequest ensureFlushBubblesRequest() => $_ensure(20);

  @$pb.TagNumber(44)
  AppVersionRequest get appVersionRequest => $_getN(21);
  @$pb.TagNumber(44)
  set appVersionRequest(AppVersionRequest value) => $_setField(44, value);
  @$pb.TagNumber(44)
  $core.bool hasAppVersionRequest() => $_has(21);
  @$pb.TagNumber(44)
  void clearAppVersionRequest() => $_clearField(44);
  @$pb.TagNumber(44)
  AppVersionRequest ensureAppVersionRequest() => $_ensure(21);

  @$pb.TagNumber(45)
  AppVersionResponse get appVersionResponse => $_getN(22);
  @$pb.TagNumber(45)
  set appVersionResponse(AppVersionResponse value) => $_setField(45, value);
  @$pb.TagNumber(45)
  $core.bool hasAppVersionResponse() => $_has(22);
  @$pb.TagNumber(45)
  void clearAppVersionResponse() => $_clearField(45);
  @$pb.TagNumber(45)
  AppVersionResponse ensureAppVersionResponse() => $_ensure(22);

  @$pb.TagNumber(99)
  Error get error => $_getN(23);
  @$pb.TagNumber(99)
  set error(Error value) => $_setField(99, value);
  @$pb.TagNumber(99)
  $core.bool hasError() => $_has(23);
  @$pb.TagNumber(99)
  void clearError() => $_clearField(99);
  @$pb.TagNumber(99)
  Error ensureError() => $_ensure(23);
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

/// 设置快照的 typed 模型。字段与 core settings_snapshot 的稳定键一一对应;
/// 不再用动态 map 承载整份设置。
class AppSettings extends $pb.GeneratedMessage {
  factory AppSettings({
    $core.int? color,
    $core.int? width,
    $core.bool? launchAtLogin,
    $core.bool? frostedGlass,
    $core.bool? grid,
    $core.bool? gridFollowStrokes,
    $core.bool? glassFollowStrokes,
    $core.bool? softShadow,
    $core.bool? invertInk,
    $core.bool? notebookStyle,
    $core.bool? panelTransparent,
    $core.int? invertFps,
    $core.bool? pressureMonitor,
    $core.bool? outline,
    $core.bool? infiniteCanvas,
    $core.bool? minimap,
    $core.double? gridSize,
    $core.int? gridDivider,
    $core.int? flipEffect,
    $core.int? gifFps,
    $core.double? gifResolution,
    $core.double? gifSpeed,
    $core.int? gifEndMode,
    $core.String? chatApiBase,
    $core.String? chatUser,
    $core.bool? chatHasPassword,
    $core.bool? chatIntegration,
    $core.bool? showFreeCanvas,
    $core.bool? showBubbleTab,
    $core.int? bubbleReplaySpeed,
    $core.bool? bubbleReplayHoldLast,
    $core.bool? bubbleReplayAuto,
    $core.bool? bubbleAutoNewPage,
    $core.String? nasUrl,
    $core.bool? shareCanvas,
    $core.String? language,
    $core.String? shortcuts,
  }) {
    final result = AppSettings._();
    if (color != null) result.color = color;
    if (width != null) result.width = width;
    if (launchAtLogin != null) result.launchAtLogin = launchAtLogin;
    if (frostedGlass != null) result.frostedGlass = frostedGlass;
    if (grid != null) result.grid = grid;
    if (gridFollowStrokes != null) result.gridFollowStrokes = gridFollowStrokes;
    if (glassFollowStrokes != null)
      result.glassFollowStrokes = glassFollowStrokes;
    if (softShadow != null) result.softShadow = softShadow;
    if (invertInk != null) result.invertInk = invertInk;
    if (notebookStyle != null) result.notebookStyle = notebookStyle;
    if (panelTransparent != null) result.panelTransparent = panelTransparent;
    if (invertFps != null) result.invertFps = invertFps;
    if (pressureMonitor != null) result.pressureMonitor = pressureMonitor;
    if (outline != null) result.outline = outline;
    if (infiniteCanvas != null) result.infiniteCanvas = infiniteCanvas;
    if (minimap != null) result.minimap = minimap;
    if (gridSize != null) result.gridSize = gridSize;
    if (gridDivider != null) result.gridDivider = gridDivider;
    if (flipEffect != null) result.flipEffect = flipEffect;
    if (gifFps != null) result.gifFps = gifFps;
    if (gifResolution != null) result.gifResolution = gifResolution;
    if (gifSpeed != null) result.gifSpeed = gifSpeed;
    if (gifEndMode != null) result.gifEndMode = gifEndMode;
    if (chatApiBase != null) result.chatApiBase = chatApiBase;
    if (chatUser != null) result.chatUser = chatUser;
    if (chatHasPassword != null) result.chatHasPassword = chatHasPassword;
    if (chatIntegration != null) result.chatIntegration = chatIntegration;
    if (showFreeCanvas != null) result.showFreeCanvas = showFreeCanvas;
    if (showBubbleTab != null) result.showBubbleTab = showBubbleTab;
    if (bubbleReplaySpeed != null) result.bubbleReplaySpeed = bubbleReplaySpeed;
    if (bubbleReplayHoldLast != null)
      result.bubbleReplayHoldLast = bubbleReplayHoldLast;
    if (bubbleReplayAuto != null) result.bubbleReplayAuto = bubbleReplayAuto;
    if (bubbleAutoNewPage != null) result.bubbleAutoNewPage = bubbleAutoNewPage;
    if (nasUrl != null) result.nasUrl = nasUrl;
    if (shareCanvas != null) result.shareCanvas = shareCanvas;
    if (language != null) result.language = language;
    if (shortcuts != null) result.shortcuts = shortcuts;
    return result;
  }

  AppSettings._();

  factory AppSettings.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppSettings()..mergeFromBuffer(data, registry);
  factory AppSettings.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppSettings()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'AppSettings',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: AppSettings.$_createMessage)
    ..aI(1, _omitFieldNames ? '' : 'color')
    ..aI(2, _omitFieldNames ? '' : 'width')
    ..aOB(3, _omitFieldNames ? '' : 'launchAtLogin')
    ..aOB(4, _omitFieldNames ? '' : 'frostedGlass')
    ..aOB(5, _omitFieldNames ? '' : 'grid')
    ..aOB(6, _omitFieldNames ? '' : 'gridFollowStrokes')
    ..aOB(7, _omitFieldNames ? '' : 'glassFollowStrokes')
    ..aOB(8, _omitFieldNames ? '' : 'softShadow')
    ..aOB(9, _omitFieldNames ? '' : 'invertInk')
    ..aOB(10, _omitFieldNames ? '' : 'notebookStyle')
    ..aOB(11, _omitFieldNames ? '' : 'panelTransparent')
    ..aI(12, _omitFieldNames ? '' : 'invertFps')
    ..aOB(13, _omitFieldNames ? '' : 'pressureMonitor')
    ..aOB(14, _omitFieldNames ? '' : 'outline')
    ..aOB(15, _omitFieldNames ? '' : 'infiniteCanvas')
    ..aOB(16, _omitFieldNames ? '' : 'minimap')
    ..aD(17, _omitFieldNames ? '' : 'gridSize')
    ..aI(18, _omitFieldNames ? '' : 'gridDivider')
    ..aI(19, _omitFieldNames ? '' : 'flipEffect')
    ..aI(20, _omitFieldNames ? '' : 'gifFps')
    ..aD(21, _omitFieldNames ? '' : 'gifResolution')
    ..aD(22, _omitFieldNames ? '' : 'gifSpeed')
    ..aI(23, _omitFieldNames ? '' : 'gifEndMode')
    ..aOS(24, _omitFieldNames ? '' : 'chatApiBase')
    ..aOS(25, _omitFieldNames ? '' : 'chatUser')
    ..aOB(26, _omitFieldNames ? '' : 'chatHasPassword')
    ..aOB(27, _omitFieldNames ? '' : 'chatIntegration')
    ..aOB(28, _omitFieldNames ? '' : 'showFreeCanvas')
    ..aOB(29, _omitFieldNames ? '' : 'showBubbleTab')
    ..aI(30, _omitFieldNames ? '' : 'bubbleReplaySpeed')
    ..aOB(31, _omitFieldNames ? '' : 'bubbleReplayHoldLast')
    ..aOB(32, _omitFieldNames ? '' : 'bubbleReplayAuto')
    ..aOB(33, _omitFieldNames ? '' : 'bubbleAutoNewPage')
    ..aOS(34, _omitFieldNames ? '' : 'nasUrl')
    ..aOB(35, _omitFieldNames ? '' : 'shareCanvas')
    ..aOS(36, _omitFieldNames ? '' : 'language')
    ..aOS(37, _omitFieldNames ? '' : 'shortcuts')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppSettings clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppSettings copyWith(void Function(AppSettings) updates) =>
      super.copyWith((message) => updates(message as AppSettings))
          as AppSettings;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use AppSettings() / AppSettings.new instead')
  static AppSettings create() => AppSettings._();
  static $pb.GeneratedMessage $_createMessage() => AppSettings._();
  @$core.override
  AppSettings createEmptyInstance() => AppSettings._();
  @$core.pragma('dart2js:noInline')
  static AppSettings getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<AppSettings>(
          AppSettings.$_createMessage);
  static AppSettings? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get color => $_getIZ(0);
  @$pb.TagNumber(1)
  set color($core.int value) => $_setSignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasColor() => $_has(0);
  @$pb.TagNumber(1)
  void clearColor() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get width => $_getIZ(1);
  @$pb.TagNumber(2)
  set width($core.int value) => $_setSignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasWidth() => $_has(1);
  @$pb.TagNumber(2)
  void clearWidth() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get launchAtLogin => $_getBF(2);
  @$pb.TagNumber(3)
  set launchAtLogin($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasLaunchAtLogin() => $_has(2);
  @$pb.TagNumber(3)
  void clearLaunchAtLogin() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get frostedGlass => $_getBF(3);
  @$pb.TagNumber(4)
  set frostedGlass($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasFrostedGlass() => $_has(3);
  @$pb.TagNumber(4)
  void clearFrostedGlass() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.bool get grid => $_getBF(4);
  @$pb.TagNumber(5)
  set grid($core.bool value) => $_setBool(4, value);
  @$pb.TagNumber(5)
  $core.bool hasGrid() => $_has(4);
  @$pb.TagNumber(5)
  void clearGrid() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.bool get gridFollowStrokes => $_getBF(5);
  @$pb.TagNumber(6)
  set gridFollowStrokes($core.bool value) => $_setBool(5, value);
  @$pb.TagNumber(6)
  $core.bool hasGridFollowStrokes() => $_has(5);
  @$pb.TagNumber(6)
  void clearGridFollowStrokes() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.bool get glassFollowStrokes => $_getBF(6);
  @$pb.TagNumber(7)
  set glassFollowStrokes($core.bool value) => $_setBool(6, value);
  @$pb.TagNumber(7)
  $core.bool hasGlassFollowStrokes() => $_has(6);
  @$pb.TagNumber(7)
  void clearGlassFollowStrokes() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.bool get softShadow => $_getBF(7);
  @$pb.TagNumber(8)
  set softShadow($core.bool value) => $_setBool(7, value);
  @$pb.TagNumber(8)
  $core.bool hasSoftShadow() => $_has(7);
  @$pb.TagNumber(8)
  void clearSoftShadow() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.bool get invertInk => $_getBF(8);
  @$pb.TagNumber(9)
  set invertInk($core.bool value) => $_setBool(8, value);
  @$pb.TagNumber(9)
  $core.bool hasInvertInk() => $_has(8);
  @$pb.TagNumber(9)
  void clearInvertInk() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.bool get notebookStyle => $_getBF(9);
  @$pb.TagNumber(10)
  set notebookStyle($core.bool value) => $_setBool(9, value);
  @$pb.TagNumber(10)
  $core.bool hasNotebookStyle() => $_has(9);
  @$pb.TagNumber(10)
  void clearNotebookStyle() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.bool get panelTransparent => $_getBF(10);
  @$pb.TagNumber(11)
  set panelTransparent($core.bool value) => $_setBool(10, value);
  @$pb.TagNumber(11)
  $core.bool hasPanelTransparent() => $_has(10);
  @$pb.TagNumber(11)
  void clearPanelTransparent() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.int get invertFps => $_getIZ(11);
  @$pb.TagNumber(12)
  set invertFps($core.int value) => $_setSignedInt32(11, value);
  @$pb.TagNumber(12)
  $core.bool hasInvertFps() => $_has(11);
  @$pb.TagNumber(12)
  void clearInvertFps() => $_clearField(12);

  @$pb.TagNumber(13)
  $core.bool get pressureMonitor => $_getBF(12);
  @$pb.TagNumber(13)
  set pressureMonitor($core.bool value) => $_setBool(12, value);
  @$pb.TagNumber(13)
  $core.bool hasPressureMonitor() => $_has(12);
  @$pb.TagNumber(13)
  void clearPressureMonitor() => $_clearField(13);

  @$pb.TagNumber(14)
  $core.bool get outline => $_getBF(13);
  @$pb.TagNumber(14)
  set outline($core.bool value) => $_setBool(13, value);
  @$pb.TagNumber(14)
  $core.bool hasOutline() => $_has(13);
  @$pb.TagNumber(14)
  void clearOutline() => $_clearField(14);

  @$pb.TagNumber(15)
  $core.bool get infiniteCanvas => $_getBF(14);
  @$pb.TagNumber(15)
  set infiniteCanvas($core.bool value) => $_setBool(14, value);
  @$pb.TagNumber(15)
  $core.bool hasInfiniteCanvas() => $_has(14);
  @$pb.TagNumber(15)
  void clearInfiniteCanvas() => $_clearField(15);

  @$pb.TagNumber(16)
  $core.bool get minimap => $_getBF(15);
  @$pb.TagNumber(16)
  set minimap($core.bool value) => $_setBool(15, value);
  @$pb.TagNumber(16)
  $core.bool hasMinimap() => $_has(15);
  @$pb.TagNumber(16)
  void clearMinimap() => $_clearField(16);

  @$pb.TagNumber(17)
  $core.double get gridSize => $_getN(16);
  @$pb.TagNumber(17)
  set gridSize($core.double value) => $_setDouble(16, value);
  @$pb.TagNumber(17)
  $core.bool hasGridSize() => $_has(16);
  @$pb.TagNumber(17)
  void clearGridSize() => $_clearField(17);

  @$pb.TagNumber(18)
  $core.int get gridDivider => $_getIZ(17);
  @$pb.TagNumber(18)
  set gridDivider($core.int value) => $_setSignedInt32(17, value);
  @$pb.TagNumber(18)
  $core.bool hasGridDivider() => $_has(17);
  @$pb.TagNumber(18)
  void clearGridDivider() => $_clearField(18);

  @$pb.TagNumber(19)
  $core.int get flipEffect => $_getIZ(18);
  @$pb.TagNumber(19)
  set flipEffect($core.int value) => $_setSignedInt32(18, value);
  @$pb.TagNumber(19)
  $core.bool hasFlipEffect() => $_has(18);
  @$pb.TagNumber(19)
  void clearFlipEffect() => $_clearField(19);

  @$pb.TagNumber(20)
  $core.int get gifFps => $_getIZ(19);
  @$pb.TagNumber(20)
  set gifFps($core.int value) => $_setSignedInt32(19, value);
  @$pb.TagNumber(20)
  $core.bool hasGifFps() => $_has(19);
  @$pb.TagNumber(20)
  void clearGifFps() => $_clearField(20);

  @$pb.TagNumber(21)
  $core.double get gifResolution => $_getN(20);
  @$pb.TagNumber(21)
  set gifResolution($core.double value) => $_setDouble(20, value);
  @$pb.TagNumber(21)
  $core.bool hasGifResolution() => $_has(20);
  @$pb.TagNumber(21)
  void clearGifResolution() => $_clearField(21);

  @$pb.TagNumber(22)
  $core.double get gifSpeed => $_getN(21);
  @$pb.TagNumber(22)
  set gifSpeed($core.double value) => $_setDouble(21, value);
  @$pb.TagNumber(22)
  $core.bool hasGifSpeed() => $_has(21);
  @$pb.TagNumber(22)
  void clearGifSpeed() => $_clearField(22);

  @$pb.TagNumber(23)
  $core.int get gifEndMode => $_getIZ(22);
  @$pb.TagNumber(23)
  set gifEndMode($core.int value) => $_setSignedInt32(22, value);
  @$pb.TagNumber(23)
  $core.bool hasGifEndMode() => $_has(22);
  @$pb.TagNumber(23)
  void clearGifEndMode() => $_clearField(23);

  @$pb.TagNumber(24)
  $core.String get chatApiBase => $_getSZ(23);
  @$pb.TagNumber(24)
  set chatApiBase($core.String value) => $_setString(23, value);
  @$pb.TagNumber(24)
  $core.bool hasChatApiBase() => $_has(23);
  @$pb.TagNumber(24)
  void clearChatApiBase() => $_clearField(24);

  @$pb.TagNumber(25)
  $core.String get chatUser => $_getSZ(24);
  @$pb.TagNumber(25)
  set chatUser($core.String value) => $_setString(24, value);
  @$pb.TagNumber(25)
  $core.bool hasChatUser() => $_has(24);
  @$pb.TagNumber(25)
  void clearChatUser() => $_clearField(25);

  @$pb.TagNumber(26)
  $core.bool get chatHasPassword => $_getBF(25);
  @$pb.TagNumber(26)
  set chatHasPassword($core.bool value) => $_setBool(25, value);
  @$pb.TagNumber(26)
  $core.bool hasChatHasPassword() => $_has(25);
  @$pb.TagNumber(26)
  void clearChatHasPassword() => $_clearField(26);

  @$pb.TagNumber(27)
  $core.bool get chatIntegration => $_getBF(26);
  @$pb.TagNumber(27)
  set chatIntegration($core.bool value) => $_setBool(26, value);
  @$pb.TagNumber(27)
  $core.bool hasChatIntegration() => $_has(26);
  @$pb.TagNumber(27)
  void clearChatIntegration() => $_clearField(27);

  @$pb.TagNumber(28)
  $core.bool get showFreeCanvas => $_getBF(27);
  @$pb.TagNumber(28)
  set showFreeCanvas($core.bool value) => $_setBool(27, value);
  @$pb.TagNumber(28)
  $core.bool hasShowFreeCanvas() => $_has(27);
  @$pb.TagNumber(28)
  void clearShowFreeCanvas() => $_clearField(28);

  @$pb.TagNumber(29)
  $core.bool get showBubbleTab => $_getBF(28);
  @$pb.TagNumber(29)
  set showBubbleTab($core.bool value) => $_setBool(28, value);
  @$pb.TagNumber(29)
  $core.bool hasShowBubbleTab() => $_has(28);
  @$pb.TagNumber(29)
  void clearShowBubbleTab() => $_clearField(29);

  @$pb.TagNumber(30)
  $core.int get bubbleReplaySpeed => $_getIZ(29);
  @$pb.TagNumber(30)
  set bubbleReplaySpeed($core.int value) => $_setSignedInt32(29, value);
  @$pb.TagNumber(30)
  $core.bool hasBubbleReplaySpeed() => $_has(29);
  @$pb.TagNumber(30)
  void clearBubbleReplaySpeed() => $_clearField(30);

  @$pb.TagNumber(31)
  $core.bool get bubbleReplayHoldLast => $_getBF(30);
  @$pb.TagNumber(31)
  set bubbleReplayHoldLast($core.bool value) => $_setBool(30, value);
  @$pb.TagNumber(31)
  $core.bool hasBubbleReplayHoldLast() => $_has(30);
  @$pb.TagNumber(31)
  void clearBubbleReplayHoldLast() => $_clearField(31);

  @$pb.TagNumber(32)
  $core.bool get bubbleReplayAuto => $_getBF(31);
  @$pb.TagNumber(32)
  set bubbleReplayAuto($core.bool value) => $_setBool(31, value);
  @$pb.TagNumber(32)
  $core.bool hasBubbleReplayAuto() => $_has(31);
  @$pb.TagNumber(32)
  void clearBubbleReplayAuto() => $_clearField(32);

  @$pb.TagNumber(33)
  $core.bool get bubbleAutoNewPage => $_getBF(32);
  @$pb.TagNumber(33)
  set bubbleAutoNewPage($core.bool value) => $_setBool(32, value);
  @$pb.TagNumber(33)
  $core.bool hasBubbleAutoNewPage() => $_has(32);
  @$pb.TagNumber(33)
  void clearBubbleAutoNewPage() => $_clearField(33);

  @$pb.TagNumber(34)
  $core.String get nasUrl => $_getSZ(33);
  @$pb.TagNumber(34)
  set nasUrl($core.String value) => $_setString(33, value);
  @$pb.TagNumber(34)
  $core.bool hasNasUrl() => $_has(33);
  @$pb.TagNumber(34)
  void clearNasUrl() => $_clearField(34);

  @$pb.TagNumber(35)
  $core.bool get shareCanvas => $_getBF(34);
  @$pb.TagNumber(35)
  set shareCanvas($core.bool value) => $_setBool(34, value);
  @$pb.TagNumber(35)
  $core.bool hasShareCanvas() => $_has(34);
  @$pb.TagNumber(35)
  void clearShareCanvas() => $_clearField(35);

  @$pb.TagNumber(36)
  $core.String get language => $_getSZ(35);
  @$pb.TagNumber(36)
  set language($core.String value) => $_setString(35, value);
  @$pb.TagNumber(36)
  $core.bool hasLanguage() => $_has(35);
  @$pb.TagNumber(36)
  void clearLanguage() => $_clearField(36);

  @$pb.TagNumber(37)
  $core.String get shortcuts => $_getSZ(36);
  @$pb.TagNumber(37)
  set shortcuts($core.String value) => $_setString(36, value);
  @$pb.TagNumber(37)
  $core.bool hasShortcuts() => $_has(36);
  @$pb.TagNumber(37)
  void clearShortcuts() => $_clearField(37);
}

class SettingsSnapshot extends $pb.GeneratedMessage {
  factory SettingsSnapshot({
    AppSettings? settings,
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? metadata,
  }) {
    final result = SettingsSnapshot._();
    if (settings != null) result.settings = settings;
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
    ..aOM<AppSettings>(1, _omitFieldNames ? '' : 'settings',
        subBuilder: AppSettings.$_createMessage)
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
  AppSettings get settings => $_getN(0);
  @$pb.TagNumber(1)
  set settings(AppSettings value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasSettings() => $_has(0);
  @$pb.TagNumber(1)
  void clearSettings() => $_clearField(1);
  @$pb.TagNumber(1)
  AppSettings ensureSettings() => $_ensure(0);

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

/// Swift 与 Rust 之间的字符串表(快捷键表/菜单标题)。
class StringMap extends $pb.GeneratedMessage {
  factory StringMap({
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? values,
  }) {
    final result = StringMap._();
    if (values != null) result.values.addEntries(values);
    return result;
  }

  StringMap._();

  factory StringMap.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      StringMap()..mergeFromBuffer(data, registry);
  factory StringMap.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      StringMap()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'StringMap',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: StringMap.$_createMessage)
    ..m<$core.String, $core.String>(1, _omitFieldNames ? '' : 'values',
        entryClassName: 'StringMap.ValuesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StringMap clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StringMap copyWith(void Function(StringMap) updates) =>
      super.copyWith((message) => updates(message as StringMap)) as StringMap;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use StringMap() / StringMap.new instead')
  static StringMap create() => StringMap._();
  static $pb.GeneratedMessage $_createMessage() => StringMap._();
  @$core.override
  StringMap createEmptyInstance() => StringMap._();
  @$core.pragma('dart2js:noInline')
  static StringMap getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<StringMap>(StringMap.$_createMessage);
  static StringMap? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbMap<$core.String, $core.String> get values => $_getMap(0);
}

class StringList extends $pb.GeneratedMessage {
  factory StringList({
    $core.Iterable<$core.String>? values,
  }) {
    final result = StringList._();
    if (values != null) result.values.addAll(values);
    return result;
  }

  StringList._();

  factory StringList.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      StringList()..mergeFromBuffer(data, registry);
  factory StringList.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      StringList()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'StringList',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: StringList.$_createMessage)
    ..pPS(1, _omitFieldNames ? '' : 'values')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StringList clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StringList copyWith(void Function(StringList) updates) =>
      super.copyWith((message) => updates(message as StringList)) as StringList;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use StringList() / StringList.new instead')
  static StringList create() => StringList._();
  static $pb.GeneratedMessage $_createMessage() => StringList._();
  @$core.override
  StringList createEmptyInstance() => StringList._();
  @$core.pragma('dart2js:noInline')
  static StringList getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<StringList>(StringList.$_createMessage);
  static StringList? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<$core.String> get values => $_getList(0);
}

class SettingsChanged extends $pb.GeneratedMessage {
  factory SettingsChanged({
    SettingsSnapshot? snapshot,
  }) {
    final result = SettingsChanged._();
    if (snapshot != null) result.snapshot = snapshot;
    return result;
  }

  SettingsChanged._();

  factory SettingsChanged.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsChanged()..mergeFromBuffer(data, registry);
  factory SettingsChanged.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsChanged()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingsChanged',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingsChanged.$_createMessage)
    ..aOM<SettingsSnapshot>(1, _omitFieldNames ? '' : 'snapshot',
        subBuilder: SettingsSnapshot.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsChanged clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsChanged copyWith(void Function(SettingsChanged) updates) =>
      super.copyWith((message) => updates(message as SettingsChanged))
          as SettingsChanged;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use SettingsChanged() / SettingsChanged.new instead')
  static SettingsChanged create() => SettingsChanged._();
  static $pb.GeneratedMessage $_createMessage() => SettingsChanged._();
  @$core.override
  SettingsChanged createEmptyInstance() => SettingsChanged._();
  @$core.pragma('dart2js:noInline')
  static SettingsChanged getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<SettingsChanged>(
          SettingsChanged.$_createMessage);
  static SettingsChanged? _defaultInstance;

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

class PageSummary extends $pb.GeneratedMessage {
  factory PageSummary({
    $fixnum.Int64? id,
    $core.int? width,
    $core.int? height,
    $core.int? strokeCount,
  }) {
    final result = PageSummary._();
    if (id != null) result.id = id;
    if (width != null) result.width = width;
    if (height != null) result.height = height;
    if (strokeCount != null) result.strokeCount = strokeCount;
    return result;
  }

  PageSummary._();

  factory PageSummary.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageSummary()..mergeFromBuffer(data, registry);
  factory PageSummary.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageSummary()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageSummary',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageSummary.$_createMessage)
    ..aInt64(1, _omitFieldNames ? '' : 'id')
    ..aI(2, _omitFieldNames ? '' : 'width')
    ..aI(3, _omitFieldNames ? '' : 'height')
    ..aI(4, _omitFieldNames ? '' : 'strokeCount')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageSummary clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageSummary copyWith(void Function(PageSummary) updates) =>
      super.copyWith((message) => updates(message as PageSummary))
          as PageSummary;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PageSummary() / PageSummary.new instead')
  static PageSummary create() => PageSummary._();
  static $pb.GeneratedMessage $_createMessage() => PageSummary._();
  @$core.override
  PageSummary createEmptyInstance() => PageSummary._();
  @$core.pragma('dart2js:noInline')
  static PageSummary getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PageSummary>(
          PageSummary.$_createMessage);
  static PageSummary? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get id => $_getI64(0);
  @$pb.TagNumber(1)
  set id($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasId() => $_has(0);
  @$pb.TagNumber(1)
  void clearId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get width => $_getIZ(1);
  @$pb.TagNumber(2)
  set width($core.int value) => $_setSignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasWidth() => $_has(1);
  @$pb.TagNumber(2)
  void clearWidth() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get height => $_getIZ(2);
  @$pb.TagNumber(3)
  set height($core.int value) => $_setSignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasHeight() => $_has(2);
  @$pb.TagNumber(3)
  void clearHeight() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.int get strokeCount => $_getIZ(3);
  @$pb.TagNumber(4)
  set strokeCount($core.int value) => $_setSignedInt32(3, value);
  @$pb.TagNumber(4)
  $core.bool hasStrokeCount() => $_has(3);
  @$pb.TagNumber(4)
  void clearStrokeCount() => $_clearField(4);
}

class PageListResponse extends $pb.GeneratedMessage {
  factory PageListResponse({
    $core.Iterable<PageSummary>? pages,
  }) {
    final result = PageListResponse._();
    if (pages != null) result.pages.addAll(pages);
    return result;
  }

  PageListResponse._();

  factory PageListResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageListResponse()..mergeFromBuffer(data, registry);
  factory PageListResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageListResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageListResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageListResponse.$_createMessage)
    ..pPM<PageSummary>(1, _omitFieldNames ? '' : 'pages',
        subBuilder: PageSummary.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageListResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageListResponse copyWith(void Function(PageListResponse) updates) =>
      super.copyWith((message) => updates(message as PageListResponse))
          as PageListResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PageListResponse() / PageListResponse.new instead')
  static PageListResponse create() => PageListResponse._();
  static $pb.GeneratedMessage $_createMessage() => PageListResponse._();
  @$core.override
  PageListResponse createEmptyInstance() => PageListResponse._();
  @$core.pragma('dart2js:noInline')
  static PageListResponse getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PageListResponse>(
          PageListResponse.$_createMessage);
  static PageListResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<PageSummary> get pages => $_getList(0);
}

class PageThumbnail extends $pb.GeneratedMessage {
  factory PageThumbnail({
    $fixnum.Int64? id,
    $core.List<$core.int>? png,
  }) {
    final result = PageThumbnail._();
    if (id != null) result.id = id;
    if (png != null) result.png = png;
    return result;
  }

  PageThumbnail._();

  factory PageThumbnail.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnail()..mergeFromBuffer(data, registry);
  factory PageThumbnail.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnail()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageThumbnail',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageThumbnail.$_createMessage)
    ..aInt64(1, _omitFieldNames ? '' : 'id')
    ..a<$core.List<$core.int>>(
        2, _omitFieldNames ? '' : 'png', $pb.PbFieldType.OY)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnail clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnail copyWith(void Function(PageThumbnail) updates) =>
      super.copyWith((message) => updates(message as PageThumbnail))
          as PageThumbnail;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PageThumbnail() / PageThumbnail.new instead')
  static PageThumbnail create() => PageThumbnail._();
  static $pb.GeneratedMessage $_createMessage() => PageThumbnail._();
  @$core.override
  PageThumbnail createEmptyInstance() => PageThumbnail._();
  @$core.pragma('dart2js:noInline')
  static PageThumbnail getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PageThumbnail>(
          PageThumbnail.$_createMessage);
  static PageThumbnail? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get id => $_getI64(0);
  @$pb.TagNumber(1)
  set id($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasId() => $_has(0);
  @$pb.TagNumber(1)
  void clearId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.List<$core.int> get png => $_getN(1);
  @$pb.TagNumber(2)
  set png($core.List<$core.int> value) => $_setBytes(1, value);
  @$pb.TagNumber(2)
  $core.bool hasPng() => $_has(1);
  @$pb.TagNumber(2)
  void clearPng() => $_clearField(2);
}

class PageThumbnailsResponse extends $pb.GeneratedMessage {
  factory PageThumbnailsResponse({
    $core.Iterable<PageThumbnail>? thumbnails,
  }) {
    final result = PageThumbnailsResponse._();
    if (thumbnails != null) result.thumbnails.addAll(thumbnails);
    return result;
  }

  PageThumbnailsResponse._();

  factory PageThumbnailsResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnailsResponse()..mergeFromBuffer(data, registry);
  factory PageThumbnailsResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnailsResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageThumbnailsResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageThumbnailsResponse.$_createMessage)
    ..pPM<PageThumbnail>(1, _omitFieldNames ? '' : 'thumbnails',
        subBuilder: PageThumbnail.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnailsResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnailsResponse copyWith(
          void Function(PageThumbnailsResponse) updates) =>
      super.copyWith((message) => updates(message as PageThumbnailsResponse))
          as PageThumbnailsResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use PageThumbnailsResponse() / PageThumbnailsResponse.new instead')
  static PageThumbnailsResponse create() => PageThumbnailsResponse._();
  static $pb.GeneratedMessage $_createMessage() => PageThumbnailsResponse._();
  @$core.override
  PageThumbnailsResponse createEmptyInstance() => PageThumbnailsResponse._();
  @$core.pragma('dart2js:noInline')
  static PageThumbnailsResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<PageThumbnailsResponse>(
          PageThumbnailsResponse.$_createMessage);
  static PageThumbnailsResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<PageThumbnail> get thumbnails => $_getList(0);
}

class BubblePoint extends $pb.GeneratedMessage {
  factory BubblePoint({
    $core.double? x,
    $core.double? y,
    $core.double? width,
  }) {
    final result = BubblePoint._();
    if (x != null) result.x = x;
    if (y != null) result.y = y;
    if (width != null) result.width = width;
    return result;
  }

  BubblePoint._();

  factory BubblePoint.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubblePoint()..mergeFromBuffer(data, registry);
  factory BubblePoint.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubblePoint()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BubblePoint',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BubblePoint.$_createMessage)
    ..aD(1, _omitFieldNames ? '' : 'x')
    ..aD(2, _omitFieldNames ? '' : 'y')
    ..aD(3, _omitFieldNames ? '' : 'width')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubblePoint clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubblePoint copyWith(void Function(BubblePoint) updates) =>
      super.copyWith((message) => updates(message as BubblePoint))
          as BubblePoint;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use BubblePoint() / BubblePoint.new instead')
  static BubblePoint create() => BubblePoint._();
  static $pb.GeneratedMessage $_createMessage() => BubblePoint._();
  @$core.override
  BubblePoint createEmptyInstance() => BubblePoint._();
  @$core.pragma('dart2js:noInline')
  static BubblePoint getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<BubblePoint>(
          BubblePoint.$_createMessage);
  static BubblePoint? _defaultInstance;

  @$pb.TagNumber(1)
  $core.double get x => $_getN(0);
  @$pb.TagNumber(1)
  set x($core.double value) => $_setDouble(0, value);
  @$pb.TagNumber(1)
  $core.bool hasX() => $_has(0);
  @$pb.TagNumber(1)
  void clearX() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.double get y => $_getN(1);
  @$pb.TagNumber(2)
  set y($core.double value) => $_setDouble(1, value);
  @$pb.TagNumber(2)
  $core.bool hasY() => $_has(1);
  @$pb.TagNumber(2)
  void clearY() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.double get width => $_getN(2);
  @$pb.TagNumber(3)
  set width($core.double value) => $_setDouble(2, value);
  @$pb.TagNumber(3)
  $core.bool hasWidth() => $_has(2);
  @$pb.TagNumber(3)
  void clearWidth() => $_clearField(3);
}

class BubbleStroke extends $pb.GeneratedMessage {
  factory BubbleStroke({
    $core.int? rgb,
    $core.double? widthScale,
    $fixnum.Int64? gapMs,
    $core.Iterable<BubblePoint>? points,
  }) {
    final result = BubbleStroke._();
    if (rgb != null) result.rgb = rgb;
    if (widthScale != null) result.widthScale = widthScale;
    if (gapMs != null) result.gapMs = gapMs;
    if (points != null) result.points.addAll(points);
    return result;
  }

  BubbleStroke._();

  factory BubbleStroke.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleStroke()..mergeFromBuffer(data, registry);
  factory BubbleStroke.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleStroke()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BubbleStroke',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BubbleStroke.$_createMessage)
    ..aI(1, _omitFieldNames ? '' : 'rgb', fieldType: $pb.PbFieldType.OU3)
    ..aD(2, _omitFieldNames ? '' : 'widthScale')
    ..a<$fixnum.Int64>(3, _omitFieldNames ? '' : 'gapMs', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..pPM<BubblePoint>(4, _omitFieldNames ? '' : 'points',
        subBuilder: BubblePoint.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleStroke clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleStroke copyWith(void Function(BubbleStroke) updates) =>
      super.copyWith((message) => updates(message as BubbleStroke))
          as BubbleStroke;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use BubbleStroke() / BubbleStroke.new instead')
  static BubbleStroke create() => BubbleStroke._();
  static $pb.GeneratedMessage $_createMessage() => BubbleStroke._();
  @$core.override
  BubbleStroke createEmptyInstance() => BubbleStroke._();
  @$core.pragma('dart2js:noInline')
  static BubbleStroke getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<BubbleStroke>(
          BubbleStroke.$_createMessage);
  static BubbleStroke? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get rgb => $_getIZ(0);
  @$pb.TagNumber(1)
  set rgb($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRgb() => $_has(0);
  @$pb.TagNumber(1)
  void clearRgb() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.double get widthScale => $_getN(1);
  @$pb.TagNumber(2)
  set widthScale($core.double value) => $_setDouble(1, value);
  @$pb.TagNumber(2)
  $core.bool hasWidthScale() => $_has(1);
  @$pb.TagNumber(2)
  void clearWidthScale() => $_clearField(2);

  @$pb.TagNumber(3)
  $fixnum.Int64 get gapMs => $_getI64(2);
  @$pb.TagNumber(3)
  set gapMs($fixnum.Int64 value) => $_setInt64(2, value);
  @$pb.TagNumber(3)
  $core.bool hasGapMs() => $_has(2);
  @$pb.TagNumber(3)
  void clearGapMs() => $_clearField(3);

  @$pb.TagNumber(4)
  $pb.PbList<BubblePoint> get points => $_getList(3);
}

class BubbleMessage extends $pb.GeneratedMessage {
  factory BubbleMessage({
    $fixnum.Int64? id,
    $fixnum.Int64? tsMs,
    $core.String? device,
    $core.Iterable<BubbleStroke>? strokes,
  }) {
    final result = BubbleMessage._();
    if (id != null) result.id = id;
    if (tsMs != null) result.tsMs = tsMs;
    if (device != null) result.device = device;
    if (strokes != null) result.strokes.addAll(strokes);
    return result;
  }

  BubbleMessage._();

  factory BubbleMessage.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleMessage()..mergeFromBuffer(data, registry);
  factory BubbleMessage.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleMessage()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BubbleMessage',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BubbleMessage.$_createMessage)
    ..a<$fixnum.Int64>(1, _omitFieldNames ? '' : 'id', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(2, _omitFieldNames ? '' : 'tsMs', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aOS(3, _omitFieldNames ? '' : 'device')
    ..pPM<BubbleStroke>(4, _omitFieldNames ? '' : 'strokes',
        subBuilder: BubbleStroke.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleMessage clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleMessage copyWith(void Function(BubbleMessage) updates) =>
      super.copyWith((message) => updates(message as BubbleMessage))
          as BubbleMessage;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use BubbleMessage() / BubbleMessage.new instead')
  static BubbleMessage create() => BubbleMessage._();
  static $pb.GeneratedMessage $_createMessage() => BubbleMessage._();
  @$core.override
  BubbleMessage createEmptyInstance() => BubbleMessage._();
  @$core.pragma('dart2js:noInline')
  static BubbleMessage getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<BubbleMessage>(
          BubbleMessage.$_createMessage);
  static BubbleMessage? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get id => $_getI64(0);
  @$pb.TagNumber(1)
  set id($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasId() => $_has(0);
  @$pb.TagNumber(1)
  void clearId() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get tsMs => $_getI64(1);
  @$pb.TagNumber(2)
  set tsMs($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTsMs() => $_has(1);
  @$pb.TagNumber(2)
  void clearTsMs() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get device => $_getSZ(2);
  @$pb.TagNumber(3)
  set device($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasDevice() => $_has(2);
  @$pb.TagNumber(3)
  void clearDevice() => $_clearField(3);

  @$pb.TagNumber(4)
  $pb.PbList<BubbleStroke> get strokes => $_getList(3);
}

class BubbleMessages extends $pb.GeneratedMessage {
  factory BubbleMessages({
    $core.String? device,
    $core.Iterable<BubbleMessage>? messages,
  }) {
    final result = BubbleMessages._();
    if (device != null) result.device = device;
    if (messages != null) result.messages.addAll(messages);
    return result;
  }

  BubbleMessages._();

  factory BubbleMessages.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleMessages()..mergeFromBuffer(data, registry);
  factory BubbleMessages.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BubbleMessages()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BubbleMessages',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BubbleMessages.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'device')
    ..pPM<BubbleMessage>(2, _omitFieldNames ? '' : 'messages',
        subBuilder: BubbleMessage.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleMessages clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BubbleMessages copyWith(void Function(BubbleMessages) updates) =>
      super.copyWith((message) => updates(message as BubbleMessages))
          as BubbleMessages;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use BubbleMessages() / BubbleMessages.new instead')
  static BubbleMessages create() => BubbleMessages._();
  static $pb.GeneratedMessage $_createMessage() => BubbleMessages._();
  @$core.override
  BubbleMessages createEmptyInstance() => BubbleMessages._();
  @$core.pragma('dart2js:noInline')
  static BubbleMessages getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<BubbleMessages>(
          BubbleMessages.$_createMessage);
  static BubbleMessages? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get device => $_getSZ(0);
  @$pb.TagNumber(1)
  set device($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasDevice() => $_has(0);
  @$pb.TagNumber(1)
  void clearDevice() => $_clearField(1);

  @$pb.TagNumber(2)
  $pb.PbList<BubbleMessage> get messages => $_getList(1);
}

class Int64List extends $pb.GeneratedMessage {
  factory Int64List({
    $core.Iterable<$fixnum.Int64>? values,
  }) {
    final result = Int64List._();
    if (values != null) result.values.addAll(values);
    return result;
  }

  Int64List._();

  factory Int64List.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Int64List()..mergeFromBuffer(data, registry);
  factory Int64List.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      Int64List()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Int64List',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: Int64List.$_createMessage)
    ..p<$fixnum.Int64>(1, _omitFieldNames ? '' : 'values', $pb.PbFieldType.K6)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Int64List clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Int64List copyWith(void Function(Int64List) updates) =>
      super.copyWith((message) => updates(message as Int64List)) as Int64List;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use Int64List() / Int64List.new instead')
  static Int64List create() => Int64List._();
  static $pb.GeneratedMessage $_createMessage() => Int64List._();
  @$core.override
  Int64List createEmptyInstance() => Int64List._();
  @$core.pragma('dart2js:noInline')
  static Int64List getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<Int64List>(Int64List.$_createMessage);
  static Int64List? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<$fixnum.Int64> get values => $_getList(0);
}

enum PageOpResponse_Result { ok, ids, payload, count, notSet }

class PageOpResponse extends $pb.GeneratedMessage {
  factory PageOpResponse({
    $core.String? kind,
    $core.bool? ok,
    Int64List? ids,
    $core.String? payload,
    $core.int? count,
  }) {
    final result = PageOpResponse._();
    if (kind != null) result.kind = kind;
    if (ok != null) result.ok = ok;
    if (ids != null) result.ids = ids;
    if (payload != null) result.payload = payload;
    if (count != null) result.count = count;
    return result;
  }

  PageOpResponse._();

  factory PageOpResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageOpResponse()..mergeFromBuffer(data, registry);
  factory PageOpResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageOpResponse()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, PageOpResponse_Result>
      _PageOpResponse_ResultByTag = {
    2: PageOpResponse_Result.ok,
    3: PageOpResponse_Result.ids,
    4: PageOpResponse_Result.payload,
    5: PageOpResponse_Result.count,
    0: PageOpResponse_Result.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageOpResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageOpResponse.$_createMessage)
    ..oo(0, [2, 3, 4, 5])
    ..aOS(1, _omitFieldNames ? '' : 'kind')
    ..aOB(2, _omitFieldNames ? '' : 'ok')
    ..aOM<Int64List>(3, _omitFieldNames ? '' : 'ids',
        subBuilder: Int64List.$_createMessage)
    ..aOS(4, _omitFieldNames ? '' : 'payload')
    ..aI(5, _omitFieldNames ? '' : 'count')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageOpResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageOpResponse copyWith(void Function(PageOpResponse) updates) =>
      super.copyWith((message) => updates(message as PageOpResponse))
          as PageOpResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PageOpResponse() / PageOpResponse.new instead')
  static PageOpResponse create() => PageOpResponse._();
  static $pb.GeneratedMessage $_createMessage() => PageOpResponse._();
  @$core.override
  PageOpResponse createEmptyInstance() => PageOpResponse._();
  @$core.pragma('dart2js:noInline')
  static PageOpResponse getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PageOpResponse>(
          PageOpResponse.$_createMessage);
  static PageOpResponse? _defaultInstance;

  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  PageOpResponse_Result whichResult() =>
      _PageOpResponse_ResultByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  void clearResult() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  $core.String get kind => $_getSZ(0);
  @$pb.TagNumber(1)
  set kind($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKind() => $_has(0);
  @$pb.TagNumber(1)
  void clearKind() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.bool get ok => $_getBF(1);
  @$pb.TagNumber(2)
  set ok($core.bool value) => $_setBool(1, value);
  @$pb.TagNumber(2)
  $core.bool hasOk() => $_has(1);
  @$pb.TagNumber(2)
  void clearOk() => $_clearField(2);

  @$pb.TagNumber(3)
  Int64List get ids => $_getN(2);
  @$pb.TagNumber(3)
  set ids(Int64List value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasIds() => $_has(2);
  @$pb.TagNumber(3)
  void clearIds() => $_clearField(3);
  @$pb.TagNumber(3)
  Int64List ensureIds() => $_ensure(2);

  @$pb.TagNumber(4)
  $core.String get payload => $_getSZ(3);
  @$pb.TagNumber(4)
  set payload($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasPayload() => $_has(3);
  @$pb.TagNumber(4)
  void clearPayload() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.int get count => $_getIZ(4);
  @$pb.TagNumber(5)
  set count($core.int value) => $_setSignedInt32(4, value);
  @$pb.TagNumber(5)
  $core.bool hasCount() => $_has(4);
  @$pb.TagNumber(5)
  void clearCount() => $_clearField(5);
}

class GetSettingsRequest extends $pb.GeneratedMessage {
  factory GetSettingsRequest() => GetSettingsRequest._();

  GetSettingsRequest._();

  factory GetSettingsRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetSettingsRequest()..mergeFromBuffer(data, registry);
  factory GetSettingsRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetSettingsRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'GetSettingsRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: GetSettingsRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetSettingsRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetSettingsRequest copyWith(void Function(GetSettingsRequest) updates) =>
      super.copyWith((message) => updates(message as GetSettingsRequest))
          as GetSettingsRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use GetSettingsRequest() / GetSettingsRequest.new instead')
  static GetSettingsRequest create() => GetSettingsRequest._();
  static $pb.GeneratedMessage $_createMessage() => GetSettingsRequest._();
  @$core.override
  GetSettingsRequest createEmptyInstance() => GetSettingsRequest._();
  @$core.pragma('dart2js:noInline')
  static GetSettingsRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<GetSettingsRequest>(
          GetSettingsRequest.$_createMessage);
  static GetSettingsRequest? _defaultInstance;
}

class ListPagesRequest extends $pb.GeneratedMessage {
  factory ListPagesRequest() => ListPagesRequest._();

  ListPagesRequest._();

  factory ListPagesRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListPagesRequest()..mergeFromBuffer(data, registry);
  factory ListPagesRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ListPagesRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListPagesRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ListPagesRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListPagesRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListPagesRequest copyWith(void Function(ListPagesRequest) updates) =>
      super.copyWith((message) => updates(message as ListPagesRequest))
          as ListPagesRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ListPagesRequest() / ListPagesRequest.new instead')
  static ListPagesRequest create() => ListPagesRequest._();
  static $pb.GeneratedMessage $_createMessage() => ListPagesRequest._();
  @$core.override
  ListPagesRequest createEmptyInstance() => ListPagesRequest._();
  @$core.pragma('dart2js:noInline')
  static ListPagesRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ListPagesRequest>(
          ListPagesRequest.$_createMessage);
  static ListPagesRequest? _defaultInstance;
}

class PageThumbnailsRequest extends $pb.GeneratedMessage {
  factory PageThumbnailsRequest({
    $core.Iterable<$fixnum.Int64>? ids,
    $core.int? maxSize,
  }) {
    final result = PageThumbnailsRequest._();
    if (ids != null) result.ids.addAll(ids);
    if (maxSize != null) result.maxSize = maxSize;
    return result;
  }

  PageThumbnailsRequest._();

  factory PageThumbnailsRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnailsRequest()..mergeFromBuffer(data, registry);
  factory PageThumbnailsRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageThumbnailsRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageThumbnailsRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageThumbnailsRequest.$_createMessage)
    ..p<$fixnum.Int64>(1, _omitFieldNames ? '' : 'ids', $pb.PbFieldType.K6)
    ..aI(2, _omitFieldNames ? '' : 'maxSize')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnailsRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageThumbnailsRequest copyWith(
          void Function(PageThumbnailsRequest) updates) =>
      super.copyWith((message) => updates(message as PageThumbnailsRequest))
          as PageThumbnailsRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use PageThumbnailsRequest() / PageThumbnailsRequest.new instead')
  static PageThumbnailsRequest create() => PageThumbnailsRequest._();
  static $pb.GeneratedMessage $_createMessage() => PageThumbnailsRequest._();
  @$core.override
  PageThumbnailsRequest createEmptyInstance() => PageThumbnailsRequest._();
  @$core.pragma('dart2js:noInline')
  static PageThumbnailsRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<PageThumbnailsRequest>(
          PageThumbnailsRequest.$_createMessage);
  static PageThumbnailsRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<$fixnum.Int64> get ids => $_getList(0);

  @$pb.TagNumber(2)
  $core.int get maxSize => $_getIZ(1);
  @$pb.TagNumber(2)
  set maxSize($core.int value) => $_setSignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMaxSize() => $_has(1);
  @$pb.TagNumber(2)
  void clearMaxSize() => $_clearField(2);
}

/// 页面操作请求: kind 区分 reorder/delete/lasso/move/...; 全 scalar/typed 字段,
/// 不再用 Panel + Struct 承载参数。
class PageOpRequest extends $pb.GeneratedMessage {
  factory PageOpRequest({
    $core.String? kind,
    $fixnum.Int64? screenId,
    $fixnum.Int64? anchorId,
    $core.bool? before,
    $core.Iterable<$fixnum.Int64>? ids,
    $core.Iterable<$core.double>? lasso,
    $fixnum.Int64? targetScreenId,
    $core.double? dx,
    $core.double? dy,
    $core.String? payload,
    $core.double? cx,
    $core.double? cy,
  }) {
    final result = PageOpRequest._();
    if (kind != null) result.kind = kind;
    if (screenId != null) result.screenId = screenId;
    if (anchorId != null) result.anchorId = anchorId;
    if (before != null) result.before = before;
    if (ids != null) result.ids.addAll(ids);
    if (lasso != null) result.lasso.addAll(lasso);
    if (targetScreenId != null) result.targetScreenId = targetScreenId;
    if (dx != null) result.dx = dx;
    if (dy != null) result.dy = dy;
    if (payload != null) result.payload = payload;
    if (cx != null) result.cx = cx;
    if (cy != null) result.cy = cy;
    return result;
  }

  PageOpRequest._();

  factory PageOpRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageOpRequest()..mergeFromBuffer(data, registry);
  factory PageOpRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      PageOpRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PageOpRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: PageOpRequest.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'kind')
    ..aInt64(2, _omitFieldNames ? '' : 'screenId')
    ..aInt64(3, _omitFieldNames ? '' : 'anchorId')
    ..aOB(4, _omitFieldNames ? '' : 'before')
    ..p<$fixnum.Int64>(5, _omitFieldNames ? '' : 'ids', $pb.PbFieldType.K6)
    ..p<$core.double>(6, _omitFieldNames ? '' : 'lasso', $pb.PbFieldType.KD)
    ..aInt64(7, _omitFieldNames ? '' : 'targetScreenId')
    ..aD(8, _omitFieldNames ? '' : 'dx')
    ..aD(9, _omitFieldNames ? '' : 'dy')
    ..aOS(10, _omitFieldNames ? '' : 'payload')
    ..aD(11, _omitFieldNames ? '' : 'cx')
    ..aD(12, _omitFieldNames ? '' : 'cy')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageOpRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PageOpRequest copyWith(void Function(PageOpRequest) updates) =>
      super.copyWith((message) => updates(message as PageOpRequest))
          as PageOpRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use PageOpRequest() / PageOpRequest.new instead')
  static PageOpRequest create() => PageOpRequest._();
  static $pb.GeneratedMessage $_createMessage() => PageOpRequest._();
  @$core.override
  PageOpRequest createEmptyInstance() => PageOpRequest._();
  @$core.pragma('dart2js:noInline')
  static PageOpRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<PageOpRequest>(
          PageOpRequest.$_createMessage);
  static PageOpRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get kind => $_getSZ(0);
  @$pb.TagNumber(1)
  set kind($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKind() => $_has(0);
  @$pb.TagNumber(1)
  void clearKind() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get screenId => $_getI64(1);
  @$pb.TagNumber(2)
  set screenId($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasScreenId() => $_has(1);
  @$pb.TagNumber(2)
  void clearScreenId() => $_clearField(2);

  @$pb.TagNumber(3)
  $fixnum.Int64 get anchorId => $_getI64(2);
  @$pb.TagNumber(3)
  set anchorId($fixnum.Int64 value) => $_setInt64(2, value);
  @$pb.TagNumber(3)
  $core.bool hasAnchorId() => $_has(2);
  @$pb.TagNumber(3)
  void clearAnchorId() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get before => $_getBF(3);
  @$pb.TagNumber(4)
  set before($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasBefore() => $_has(3);
  @$pb.TagNumber(4)
  void clearBefore() => $_clearField(4);

  @$pb.TagNumber(5)
  $pb.PbList<$fixnum.Int64> get ids => $_getList(4);

  /// lasso 多边形 x,y 平铺。
  @$pb.TagNumber(6)
  $pb.PbList<$core.double> get lasso => $_getList(5);

  @$pb.TagNumber(7)
  $fixnum.Int64 get targetScreenId => $_getI64(6);
  @$pb.TagNumber(7)
  set targetScreenId($fixnum.Int64 value) => $_setInt64(6, value);
  @$pb.TagNumber(7)
  $core.bool hasTargetScreenId() => $_has(6);
  @$pb.TagNumber(7)
  void clearTargetScreenId() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.double get dx => $_getN(7);
  @$pb.TagNumber(8)
  set dx($core.double value) => $_setDouble(7, value);
  @$pb.TagNumber(8)
  $core.bool hasDx() => $_has(7);
  @$pb.TagNumber(8)
  void clearDx() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.double get dy => $_getN(8);
  @$pb.TagNumber(9)
  set dy($core.double value) => $_setDouble(8, value);
  @$pb.TagNumber(9)
  $core.bool hasDy() => $_has(8);
  @$pb.TagNumber(9)
  void clearDy() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.String get payload => $_getSZ(9);
  @$pb.TagNumber(10)
  set payload($core.String value) => $_setString(9, value);
  @$pb.TagNumber(10)
  $core.bool hasPayload() => $_has(9);
  @$pb.TagNumber(10)
  void clearPayload() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.double get cx => $_getN(10);
  @$pb.TagNumber(11)
  set cx($core.double value) => $_setDouble(10, value);
  @$pb.TagNumber(11)
  $core.bool hasCx() => $_has(10);
  @$pb.TagNumber(11)
  void clearCx() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.double get cy => $_getN(11);
  @$pb.TagNumber(12)
  set cy($core.double value) => $_setDouble(11, value);
  @$pb.TagNumber(12)
  $core.bool hasCy() => $_has(11);
  @$pb.TagNumber(12)
  void clearCy() => $_clearField(12);
}

class GetBubblesRequest extends $pb.GeneratedMessage {
  factory GetBubblesRequest() => GetBubblesRequest._();

  GetBubblesRequest._();

  factory GetBubblesRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetBubblesRequest()..mergeFromBuffer(data, registry);
  factory GetBubblesRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      GetBubblesRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'GetBubblesRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: GetBubblesRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetBubblesRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  GetBubblesRequest copyWith(void Function(GetBubblesRequest) updates) =>
      super.copyWith((message) => updates(message as GetBubblesRequest))
          as GetBubblesRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use GetBubblesRequest() / GetBubblesRequest.new instead')
  static GetBubblesRequest create() => GetBubblesRequest._();
  static $pb.GeneratedMessage $_createMessage() => GetBubblesRequest._();
  @$core.override
  GetBubblesRequest createEmptyInstance() => GetBubblesRequest._();
  @$core.pragma('dart2js:noInline')
  static GetBubblesRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<GetBubblesRequest>(
          GetBubblesRequest.$_createMessage);
  static GetBubblesRequest? _defaultInstance;
}

class NavigateToPageRequest extends $pb.GeneratedMessage {
  factory NavigateToPageRequest({
    $fixnum.Int64? screenId,
  }) {
    final result = NavigateToPageRequest._();
    if (screenId != null) result.screenId = screenId;
    return result;
  }

  NavigateToPageRequest._();

  factory NavigateToPageRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NavigateToPageRequest()..mergeFromBuffer(data, registry);
  factory NavigateToPageRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NavigateToPageRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'NavigateToPageRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: NavigateToPageRequest.$_createMessage)
    ..aInt64(1, _omitFieldNames ? '' : 'screenId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NavigateToPageRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NavigateToPageRequest copyWith(
          void Function(NavigateToPageRequest) updates) =>
      super.copyWith((message) => updates(message as NavigateToPageRequest))
          as NavigateToPageRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use NavigateToPageRequest() / NavigateToPageRequest.new instead')
  static NavigateToPageRequest create() => NavigateToPageRequest._();
  static $pb.GeneratedMessage $_createMessage() => NavigateToPageRequest._();
  @$core.override
  NavigateToPageRequest createEmptyInstance() => NavigateToPageRequest._();
  @$core.pragma('dart2js:noInline')
  static NavigateToPageRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<NavigateToPageRequest>(
          NavigateToPageRequest.$_createMessage);
  static NavigateToPageRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get screenId => $_getI64(0);
  @$pb.TagNumber(1)
  set screenId($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasScreenId() => $_has(0);
  @$pb.TagNumber(1)
  void clearScreenId() => $_clearField(1);
}

class HotkeyCommand extends $pb.GeneratedMessage {
  factory HotkeyCommand({
    $core.String? key,
  }) {
    final result = HotkeyCommand._();
    if (key != null) result.key = key;
    return result;
  }

  HotkeyCommand._();

  factory HotkeyCommand.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      HotkeyCommand()..mergeFromBuffer(data, registry);
  factory HotkeyCommand.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      HotkeyCommand()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'HotkeyCommand',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: HotkeyCommand.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'key')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HotkeyCommand clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HotkeyCommand copyWith(void Function(HotkeyCommand) updates) =>
      super.copyWith((message) => updates(message as HotkeyCommand))
          as HotkeyCommand;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use HotkeyCommand() / HotkeyCommand.new instead')
  static HotkeyCommand create() => HotkeyCommand._();
  static $pb.GeneratedMessage $_createMessage() => HotkeyCommand._();
  @$core.override
  HotkeyCommand createEmptyInstance() => HotkeyCommand._();
  @$core.pragma('dart2js:noInline')
  static HotkeyCommand getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<HotkeyCommand>(
          HotkeyCommand.$_createMessage);
  static HotkeyCommand? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get key => $_getSZ(0);
  @$pb.TagNumber(1)
  set key($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearKey() => $_clearField(1);
}

class OpenUrlCommand extends $pb.GeneratedMessage {
  factory OpenUrlCommand({
    $core.String? url,
  }) {
    final result = OpenUrlCommand._();
    if (url != null) result.url = url;
    return result;
  }

  OpenUrlCommand._();

  factory OpenUrlCommand.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      OpenUrlCommand()..mergeFromBuffer(data, registry);
  factory OpenUrlCommand.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      OpenUrlCommand()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'OpenUrlCommand',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: OpenUrlCommand.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'url')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  OpenUrlCommand clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  OpenUrlCommand copyWith(void Function(OpenUrlCommand) updates) =>
      super.copyWith((message) => updates(message as OpenUrlCommand))
          as OpenUrlCommand;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use OpenUrlCommand() / OpenUrlCommand.new instead')
  static OpenUrlCommand create() => OpenUrlCommand._();
  static $pb.GeneratedMessage $_createMessage() => OpenUrlCommand._();
  @$core.override
  OpenUrlCommand createEmptyInstance() => OpenUrlCommand._();
  @$core.pragma('dart2js:noInline')
  static OpenUrlCommand getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<OpenUrlCommand>(
          OpenUrlCommand.$_createMessage);
  static OpenUrlCommand? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get url => $_getSZ(0);
  @$pb.TagNumber(1)
  set url($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasUrl() => $_has(0);
  @$pb.TagNumber(1)
  void clearUrl() => $_clearField(1);
}

class FlushBubblesRequest extends $pb.GeneratedMessage {
  factory FlushBubblesRequest() => FlushBubblesRequest._();

  FlushBubblesRequest._();

  factory FlushBubblesRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      FlushBubblesRequest()..mergeFromBuffer(data, registry);
  factory FlushBubblesRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      FlushBubblesRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'FlushBubblesRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: FlushBubblesRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  FlushBubblesRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  FlushBubblesRequest copyWith(void Function(FlushBubblesRequest) updates) =>
      super.copyWith((message) => updates(message as FlushBubblesRequest))
          as FlushBubblesRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core
      .Deprecated('Use FlushBubblesRequest() / FlushBubblesRequest.new instead')
  static FlushBubblesRequest create() => FlushBubblesRequest._();
  static $pb.GeneratedMessage $_createMessage() => FlushBubblesRequest._();
  @$core.override
  FlushBubblesRequest createEmptyInstance() => FlushBubblesRequest._();
  @$core.pragma('dart2js:noInline')
  static FlushBubblesRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<FlushBubblesRequest>(
          FlushBubblesRequest.$_createMessage);
  static FlushBubblesRequest? _defaultInstance;
}

class AppVersionRequest extends $pb.GeneratedMessage {
  factory AppVersionRequest() => AppVersionRequest._();

  AppVersionRequest._();

  factory AppVersionRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppVersionRequest()..mergeFromBuffer(data, registry);
  factory AppVersionRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppVersionRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'AppVersionRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: AppVersionRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppVersionRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppVersionRequest copyWith(void Function(AppVersionRequest) updates) =>
      super.copyWith((message) => updates(message as AppVersionRequest))
          as AppVersionRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use AppVersionRequest() / AppVersionRequest.new instead')
  static AppVersionRequest create() => AppVersionRequest._();
  static $pb.GeneratedMessage $_createMessage() => AppVersionRequest._();
  @$core.override
  AppVersionRequest createEmptyInstance() => AppVersionRequest._();
  @$core.pragma('dart2js:noInline')
  static AppVersionRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<AppVersionRequest>(
          AppVersionRequest.$_createMessage);
  static AppVersionRequest? _defaultInstance;
}

class AppVersionResponse extends $pb.GeneratedMessage {
  factory AppVersionResponse({
    $core.String? version,
  }) {
    final result = AppVersionResponse._();
    if (version != null) result.version = version;
    return result;
  }

  AppVersionResponse._();

  factory AppVersionResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppVersionResponse()..mergeFromBuffer(data, registry);
  factory AppVersionResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      AppVersionResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'AppVersionResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: AppVersionResponse.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'version')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppVersionResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AppVersionResponse copyWith(void Function(AppVersionResponse) updates) =>
      super.copyWith((message) => updates(message as AppVersionResponse))
          as AppVersionResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use AppVersionResponse() / AppVersionResponse.new instead')
  static AppVersionResponse create() => AppVersionResponse._();
  static $pb.GeneratedMessage $_createMessage() => AppVersionResponse._();
  @$core.override
  AppVersionResponse createEmptyInstance() => AppVersionResponse._();
  @$core.pragma('dart2js:noInline')
  static AppVersionResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<AppVersionResponse>(
          AppVersionResponse.$_createMessage);
  static AppVersionResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get version => $_getSZ(0);
  @$pb.TagNumber(1)
  set version($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasVersion() => $_has(0);
  @$pb.TagNumber(1)
  void clearVersion() => $_clearField(1);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
