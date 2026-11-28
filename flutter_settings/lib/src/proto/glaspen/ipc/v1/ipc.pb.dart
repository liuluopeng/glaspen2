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

import 'ipc.pbenum.dart';

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

export 'ipc.pbenum.dart';

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
  canvasOverviewRequest,
  canvasOverviewResponse,
  backupCommandRequest,
  backupOutcome,
  chatLoginRequest,
  chatLoginOutcome,
  exportPdfRequest,
  checkUpdateRequest,
  updateInfo,
  nasBackupRequest,
  nasBackupOutcome,
  applyUpdateRequest,
  applyUpdateOutcome,
  downloadUpdateRequest,
  downloadProgress,
  cancelDownloadRequest,
  uiConfigRequest,
  uiConfigResponse,
  settingsPatch,
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
    CanvasOverviewRequest? canvasOverviewRequest,
    CanvasOverviewResponse? canvasOverviewResponse,
    BackupCommandRequest? backupCommandRequest,
    BackupOutcome? backupOutcome,
    ChatLoginRequest? chatLoginRequest,
    ChatLoginOutcome? chatLoginOutcome,
    ExportPdfRequest? exportPdfRequest,
    CheckUpdateRequest? checkUpdateRequest,
    UpdateInfo? updateInfo,
    NasBackupRequest? nasBackupRequest,
    NasBackupOutcome? nasBackupOutcome,
    ApplyUpdateRequest? applyUpdateRequest,
    ApplyUpdateOutcome? applyUpdateOutcome,
    DownloadUpdateRequest? downloadUpdateRequest,
    DownloadProgress? downloadProgress,
    CancelDownloadRequest? cancelDownloadRequest,
    UiConfigRequest? uiConfigRequest,
    UiConfigResponse? uiConfigResponse,
    SettingsPatch? settingsPatch,
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
    if (canvasOverviewRequest != null)
      result.canvasOverviewRequest = canvasOverviewRequest;
    if (canvasOverviewResponse != null)
      result.canvasOverviewResponse = canvasOverviewResponse;
    if (backupCommandRequest != null)
      result.backupCommandRequest = backupCommandRequest;
    if (backupOutcome != null) result.backupOutcome = backupOutcome;
    if (chatLoginRequest != null) result.chatLoginRequest = chatLoginRequest;
    if (chatLoginOutcome != null) result.chatLoginOutcome = chatLoginOutcome;
    if (exportPdfRequest != null) result.exportPdfRequest = exportPdfRequest;
    if (checkUpdateRequest != null)
      result.checkUpdateRequest = checkUpdateRequest;
    if (updateInfo != null) result.updateInfo = updateInfo;
    if (nasBackupRequest != null) result.nasBackupRequest = nasBackupRequest;
    if (nasBackupOutcome != null) result.nasBackupOutcome = nasBackupOutcome;
    if (applyUpdateRequest != null)
      result.applyUpdateRequest = applyUpdateRequest;
    if (applyUpdateOutcome != null)
      result.applyUpdateOutcome = applyUpdateOutcome;
    if (downloadUpdateRequest != null)
      result.downloadUpdateRequest = downloadUpdateRequest;
    if (downloadProgress != null) result.downloadProgress = downloadProgress;
    if (cancelDownloadRequest != null)
      result.cancelDownloadRequest = cancelDownloadRequest;
    if (uiConfigRequest != null) result.uiConfigRequest = uiConfigRequest;
    if (uiConfigResponse != null) result.uiConfigResponse = uiConfigResponse;
    if (settingsPatch != null) result.settingsPatch = settingsPatch;
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
    46: Frame_Body.canvasOverviewRequest,
    47: Frame_Body.canvasOverviewResponse,
    50: Frame_Body.backupCommandRequest,
    51: Frame_Body.backupOutcome,
    52: Frame_Body.chatLoginRequest,
    53: Frame_Body.chatLoginOutcome,
    54: Frame_Body.exportPdfRequest,
    60: Frame_Body.checkUpdateRequest,
    61: Frame_Body.updateInfo,
    62: Frame_Body.nasBackupRequest,
    63: Frame_Body.nasBackupOutcome,
    64: Frame_Body.applyUpdateRequest,
    65: Frame_Body.applyUpdateOutcome,
    70: Frame_Body.downloadUpdateRequest,
    71: Frame_Body.downloadProgress,
    72: Frame_Body.cancelDownloadRequest,
    80: Frame_Body.uiConfigRequest,
    81: Frame_Body.uiConfigResponse,
    90: Frame_Body.settingsPatch,
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
      46,
      47,
      50,
      51,
      52,
      53,
      54,
      60,
      61,
      62,
      63,
      64,
      65,
      70,
      71,
      72,
      80,
      81,
      90,
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
    ..aOM<CanvasOverviewRequest>(
        46, _omitFieldNames ? '' : 'canvasOverviewRequest',
        subBuilder: CanvasOverviewRequest.$_createMessage)
    ..aOM<CanvasOverviewResponse>(
        47, _omitFieldNames ? '' : 'canvasOverviewResponse',
        subBuilder: CanvasOverviewResponse.$_createMessage)
    ..aOM<BackupCommandRequest>(
        50, _omitFieldNames ? '' : 'backupCommandRequest',
        subBuilder: BackupCommandRequest.$_createMessage)
    ..aOM<BackupOutcome>(51, _omitFieldNames ? '' : 'backupOutcome',
        subBuilder: BackupOutcome.$_createMessage)
    ..aOM<ChatLoginRequest>(52, _omitFieldNames ? '' : 'chatLoginRequest',
        subBuilder: ChatLoginRequest.$_createMessage)
    ..aOM<ChatLoginOutcome>(53, _omitFieldNames ? '' : 'chatLoginOutcome',
        subBuilder: ChatLoginOutcome.$_createMessage)
    ..aOM<ExportPdfRequest>(54, _omitFieldNames ? '' : 'exportPdfRequest',
        subBuilder: ExportPdfRequest.$_createMessage)
    ..aOM<CheckUpdateRequest>(60, _omitFieldNames ? '' : 'checkUpdateRequest',
        subBuilder: CheckUpdateRequest.$_createMessage)
    ..aOM<UpdateInfo>(61, _omitFieldNames ? '' : 'updateInfo',
        subBuilder: UpdateInfo.$_createMessage)
    ..aOM<NasBackupRequest>(62, _omitFieldNames ? '' : 'nasBackupRequest',
        subBuilder: NasBackupRequest.$_createMessage)
    ..aOM<NasBackupOutcome>(63, _omitFieldNames ? '' : 'nasBackupOutcome',
        subBuilder: NasBackupOutcome.$_createMessage)
    ..aOM<ApplyUpdateRequest>(64, _omitFieldNames ? '' : 'applyUpdateRequest',
        subBuilder: ApplyUpdateRequest.$_createMessage)
    ..aOM<ApplyUpdateOutcome>(65, _omitFieldNames ? '' : 'applyUpdateOutcome',
        subBuilder: ApplyUpdateOutcome.$_createMessage)
    ..aOM<DownloadUpdateRequest>(
        70, _omitFieldNames ? '' : 'downloadUpdateRequest',
        subBuilder: DownloadUpdateRequest.$_createMessage)
    ..aOM<DownloadProgress>(71, _omitFieldNames ? '' : 'downloadProgress',
        subBuilder: DownloadProgress.$_createMessage)
    ..aOM<CancelDownloadRequest>(
        72, _omitFieldNames ? '' : 'cancelDownloadRequest',
        subBuilder: CancelDownloadRequest.$_createMessage)
    ..aOM<UiConfigRequest>(80, _omitFieldNames ? '' : 'uiConfigRequest',
        subBuilder: UiConfigRequest.$_createMessage)
    ..aOM<UiConfigResponse>(81, _omitFieldNames ? '' : 'uiConfigResponse',
        subBuilder: UiConfigResponse.$_createMessage)
    ..aOM<SettingsPatch>(90, _omitFieldNames ? '' : 'settingsPatch',
        subBuilder: SettingsPatch.$_createMessage)
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
  @$pb.TagNumber(46)
  @$pb.TagNumber(47)
  @$pb.TagNumber(50)
  @$pb.TagNumber(51)
  @$pb.TagNumber(52)
  @$pb.TagNumber(53)
  @$pb.TagNumber(54)
  @$pb.TagNumber(60)
  @$pb.TagNumber(61)
  @$pb.TagNumber(62)
  @$pb.TagNumber(63)
  @$pb.TagNumber(64)
  @$pb.TagNumber(65)
  @$pb.TagNumber(70)
  @$pb.TagNumber(71)
  @$pb.TagNumber(72)
  @$pb.TagNumber(80)
  @$pb.TagNumber(81)
  @$pb.TagNumber(90)
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
  @$pb.TagNumber(46)
  @$pb.TagNumber(47)
  @$pb.TagNumber(50)
  @$pb.TagNumber(51)
  @$pb.TagNumber(52)
  @$pb.TagNumber(53)
  @$pb.TagNumber(54)
  @$pb.TagNumber(60)
  @$pb.TagNumber(61)
  @$pb.TagNumber(62)
  @$pb.TagNumber(63)
  @$pb.TagNumber(64)
  @$pb.TagNumber(65)
  @$pb.TagNumber(70)
  @$pb.TagNumber(71)
  @$pb.TagNumber(72)
  @$pb.TagNumber(80)
  @$pb.TagNumber(81)
  @$pb.TagNumber(90)
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

  @$pb.TagNumber(30)
  GetSettingsRequest get getSettingsRequest => $_getN(11);
  @$pb.TagNumber(30)
  set getSettingsRequest(GetSettingsRequest value) => $_setField(30, value);
  @$pb.TagNumber(30)
  $core.bool hasGetSettingsRequest() => $_has(11);
  @$pb.TagNumber(30)
  void clearGetSettingsRequest() => $_clearField(30);
  @$pb.TagNumber(30)
  GetSettingsRequest ensureGetSettingsRequest() => $_ensure(11);

  @$pb.TagNumber(31)
  ListPagesRequest get listPagesRequest => $_getN(12);
  @$pb.TagNumber(31)
  set listPagesRequest(ListPagesRequest value) => $_setField(31, value);
  @$pb.TagNumber(31)
  $core.bool hasListPagesRequest() => $_has(12);
  @$pb.TagNumber(31)
  void clearListPagesRequest() => $_clearField(31);
  @$pb.TagNumber(31)
  ListPagesRequest ensureListPagesRequest() => $_ensure(12);

  @$pb.TagNumber(32)
  PageThumbnailsRequest get pageThumbnailsRequest => $_getN(13);
  @$pb.TagNumber(32)
  set pageThumbnailsRequest(PageThumbnailsRequest value) =>
      $_setField(32, value);
  @$pb.TagNumber(32)
  $core.bool hasPageThumbnailsRequest() => $_has(13);
  @$pb.TagNumber(32)
  void clearPageThumbnailsRequest() => $_clearField(32);
  @$pb.TagNumber(32)
  PageThumbnailsRequest ensurePageThumbnailsRequest() => $_ensure(13);

  @$pb.TagNumber(33)
  PageOpRequest get pageOpRequest => $_getN(14);
  @$pb.TagNumber(33)
  set pageOpRequest(PageOpRequest value) => $_setField(33, value);
  @$pb.TagNumber(33)
  $core.bool hasPageOpRequest() => $_has(14);
  @$pb.TagNumber(33)
  void clearPageOpRequest() => $_clearField(33);
  @$pb.TagNumber(33)
  PageOpRequest ensurePageOpRequest() => $_ensure(14);

  @$pb.TagNumber(34)
  GetBubblesRequest get getBubblesRequest => $_getN(15);
  @$pb.TagNumber(34)
  set getBubblesRequest(GetBubblesRequest value) => $_setField(34, value);
  @$pb.TagNumber(34)
  $core.bool hasGetBubblesRequest() => $_has(15);
  @$pb.TagNumber(34)
  void clearGetBubblesRequest() => $_clearField(34);
  @$pb.TagNumber(34)
  GetBubblesRequest ensureGetBubblesRequest() => $_ensure(15);

  @$pb.TagNumber(40)
  NavigateToPageRequest get navigateToPageRequest => $_getN(16);
  @$pb.TagNumber(40)
  set navigateToPageRequest(NavigateToPageRequest value) =>
      $_setField(40, value);
  @$pb.TagNumber(40)
  $core.bool hasNavigateToPageRequest() => $_has(16);
  @$pb.TagNumber(40)
  void clearNavigateToPageRequest() => $_clearField(40);
  @$pb.TagNumber(40)
  NavigateToPageRequest ensureNavigateToPageRequest() => $_ensure(16);

  @$pb.TagNumber(41)
  HotkeyCommand get hotkeyCommand => $_getN(17);
  @$pb.TagNumber(41)
  set hotkeyCommand(HotkeyCommand value) => $_setField(41, value);
  @$pb.TagNumber(41)
  $core.bool hasHotkeyCommand() => $_has(17);
  @$pb.TagNumber(41)
  void clearHotkeyCommand() => $_clearField(41);
  @$pb.TagNumber(41)
  HotkeyCommand ensureHotkeyCommand() => $_ensure(17);

  @$pb.TagNumber(42)
  OpenUrlCommand get openUrlCommand => $_getN(18);
  @$pb.TagNumber(42)
  set openUrlCommand(OpenUrlCommand value) => $_setField(42, value);
  @$pb.TagNumber(42)
  $core.bool hasOpenUrlCommand() => $_has(18);
  @$pb.TagNumber(42)
  void clearOpenUrlCommand() => $_clearField(42);
  @$pb.TagNumber(42)
  OpenUrlCommand ensureOpenUrlCommand() => $_ensure(18);

  @$pb.TagNumber(43)
  FlushBubblesRequest get flushBubblesRequest => $_getN(19);
  @$pb.TagNumber(43)
  set flushBubblesRequest(FlushBubblesRequest value) => $_setField(43, value);
  @$pb.TagNumber(43)
  $core.bool hasFlushBubblesRequest() => $_has(19);
  @$pb.TagNumber(43)
  void clearFlushBubblesRequest() => $_clearField(43);
  @$pb.TagNumber(43)
  FlushBubblesRequest ensureFlushBubblesRequest() => $_ensure(19);

  @$pb.TagNumber(44)
  AppVersionRequest get appVersionRequest => $_getN(20);
  @$pb.TagNumber(44)
  set appVersionRequest(AppVersionRequest value) => $_setField(44, value);
  @$pb.TagNumber(44)
  $core.bool hasAppVersionRequest() => $_has(20);
  @$pb.TagNumber(44)
  void clearAppVersionRequest() => $_clearField(44);
  @$pb.TagNumber(44)
  AppVersionRequest ensureAppVersionRequest() => $_ensure(20);

  @$pb.TagNumber(45)
  AppVersionResponse get appVersionResponse => $_getN(21);
  @$pb.TagNumber(45)
  set appVersionResponse(AppVersionResponse value) => $_setField(45, value);
  @$pb.TagNumber(45)
  $core.bool hasAppVersionResponse() => $_has(21);
  @$pb.TagNumber(45)
  void clearAppVersionResponse() => $_clearField(45);
  @$pb.TagNumber(45)
  AppVersionResponse ensureAppVersionResponse() => $_ensure(21);

  @$pb.TagNumber(46)
  CanvasOverviewRequest get canvasOverviewRequest => $_getN(22);
  @$pb.TagNumber(46)
  set canvasOverviewRequest(CanvasOverviewRequest value) =>
      $_setField(46, value);
  @$pb.TagNumber(46)
  $core.bool hasCanvasOverviewRequest() => $_has(22);
  @$pb.TagNumber(46)
  void clearCanvasOverviewRequest() => $_clearField(46);
  @$pb.TagNumber(46)
  CanvasOverviewRequest ensureCanvasOverviewRequest() => $_ensure(22);

  @$pb.TagNumber(47)
  CanvasOverviewResponse get canvasOverviewResponse => $_getN(23);
  @$pb.TagNumber(47)
  set canvasOverviewResponse(CanvasOverviewResponse value) =>
      $_setField(47, value);
  @$pb.TagNumber(47)
  $core.bool hasCanvasOverviewResponse() => $_has(23);
  @$pb.TagNumber(47)
  void clearCanvasOverviewResponse() => $_clearField(47);
  @$pb.TagNumber(47)
  CanvasOverviewResponse ensureCanvasOverviewResponse() => $_ensure(23);

  @$pb.TagNumber(50)
  BackupCommandRequest get backupCommandRequest => $_getN(24);
  @$pb.TagNumber(50)
  set backupCommandRequest(BackupCommandRequest value) => $_setField(50, value);
  @$pb.TagNumber(50)
  $core.bool hasBackupCommandRequest() => $_has(24);
  @$pb.TagNumber(50)
  void clearBackupCommandRequest() => $_clearField(50);
  @$pb.TagNumber(50)
  BackupCommandRequest ensureBackupCommandRequest() => $_ensure(24);

  @$pb.TagNumber(51)
  BackupOutcome get backupOutcome => $_getN(25);
  @$pb.TagNumber(51)
  set backupOutcome(BackupOutcome value) => $_setField(51, value);
  @$pb.TagNumber(51)
  $core.bool hasBackupOutcome() => $_has(25);
  @$pb.TagNumber(51)
  void clearBackupOutcome() => $_clearField(51);
  @$pb.TagNumber(51)
  BackupOutcome ensureBackupOutcome() => $_ensure(25);

  @$pb.TagNumber(52)
  ChatLoginRequest get chatLoginRequest => $_getN(26);
  @$pb.TagNumber(52)
  set chatLoginRequest(ChatLoginRequest value) => $_setField(52, value);
  @$pb.TagNumber(52)
  $core.bool hasChatLoginRequest() => $_has(26);
  @$pb.TagNumber(52)
  void clearChatLoginRequest() => $_clearField(52);
  @$pb.TagNumber(52)
  ChatLoginRequest ensureChatLoginRequest() => $_ensure(26);

  @$pb.TagNumber(53)
  ChatLoginOutcome get chatLoginOutcome => $_getN(27);
  @$pb.TagNumber(53)
  set chatLoginOutcome(ChatLoginOutcome value) => $_setField(53, value);
  @$pb.TagNumber(53)
  $core.bool hasChatLoginOutcome() => $_has(27);
  @$pb.TagNumber(53)
  void clearChatLoginOutcome() => $_clearField(53);
  @$pb.TagNumber(53)
  ChatLoginOutcome ensureChatLoginOutcome() => $_ensure(27);

  @$pb.TagNumber(54)
  ExportPdfRequest get exportPdfRequest => $_getN(28);
  @$pb.TagNumber(54)
  set exportPdfRequest(ExportPdfRequest value) => $_setField(54, value);
  @$pb.TagNumber(54)
  $core.bool hasExportPdfRequest() => $_has(28);
  @$pb.TagNumber(54)
  void clearExportPdfRequest() => $_clearField(54);
  @$pb.TagNumber(54)
  ExportPdfRequest ensureExportPdfRequest() => $_ensure(28);

  @$pb.TagNumber(60)
  CheckUpdateRequest get checkUpdateRequest => $_getN(29);
  @$pb.TagNumber(60)
  set checkUpdateRequest(CheckUpdateRequest value) => $_setField(60, value);
  @$pb.TagNumber(60)
  $core.bool hasCheckUpdateRequest() => $_has(29);
  @$pb.TagNumber(60)
  void clearCheckUpdateRequest() => $_clearField(60);
  @$pb.TagNumber(60)
  CheckUpdateRequest ensureCheckUpdateRequest() => $_ensure(29);

  @$pb.TagNumber(61)
  UpdateInfo get updateInfo => $_getN(30);
  @$pb.TagNumber(61)
  set updateInfo(UpdateInfo value) => $_setField(61, value);
  @$pb.TagNumber(61)
  $core.bool hasUpdateInfo() => $_has(30);
  @$pb.TagNumber(61)
  void clearUpdateInfo() => $_clearField(61);
  @$pb.TagNumber(61)
  UpdateInfo ensureUpdateInfo() => $_ensure(30);

  @$pb.TagNumber(62)
  NasBackupRequest get nasBackupRequest => $_getN(31);
  @$pb.TagNumber(62)
  set nasBackupRequest(NasBackupRequest value) => $_setField(62, value);
  @$pb.TagNumber(62)
  $core.bool hasNasBackupRequest() => $_has(31);
  @$pb.TagNumber(62)
  void clearNasBackupRequest() => $_clearField(62);
  @$pb.TagNumber(62)
  NasBackupRequest ensureNasBackupRequest() => $_ensure(31);

  @$pb.TagNumber(63)
  NasBackupOutcome get nasBackupOutcome => $_getN(32);
  @$pb.TagNumber(63)
  set nasBackupOutcome(NasBackupOutcome value) => $_setField(63, value);
  @$pb.TagNumber(63)
  $core.bool hasNasBackupOutcome() => $_has(32);
  @$pb.TagNumber(63)
  void clearNasBackupOutcome() => $_clearField(63);
  @$pb.TagNumber(63)
  NasBackupOutcome ensureNasBackupOutcome() => $_ensure(32);

  @$pb.TagNumber(64)
  ApplyUpdateRequest get applyUpdateRequest => $_getN(33);
  @$pb.TagNumber(64)
  set applyUpdateRequest(ApplyUpdateRequest value) => $_setField(64, value);
  @$pb.TagNumber(64)
  $core.bool hasApplyUpdateRequest() => $_has(33);
  @$pb.TagNumber(64)
  void clearApplyUpdateRequest() => $_clearField(64);
  @$pb.TagNumber(64)
  ApplyUpdateRequest ensureApplyUpdateRequest() => $_ensure(33);

  @$pb.TagNumber(65)
  ApplyUpdateOutcome get applyUpdateOutcome => $_getN(34);
  @$pb.TagNumber(65)
  set applyUpdateOutcome(ApplyUpdateOutcome value) => $_setField(65, value);
  @$pb.TagNumber(65)
  $core.bool hasApplyUpdateOutcome() => $_has(34);
  @$pb.TagNumber(65)
  void clearApplyUpdateOutcome() => $_clearField(65);
  @$pb.TagNumber(65)
  ApplyUpdateOutcome ensureApplyUpdateOutcome() => $_ensure(34);

  @$pb.TagNumber(70)
  DownloadUpdateRequest get downloadUpdateRequest => $_getN(35);
  @$pb.TagNumber(70)
  set downloadUpdateRequest(DownloadUpdateRequest value) =>
      $_setField(70, value);
  @$pb.TagNumber(70)
  $core.bool hasDownloadUpdateRequest() => $_has(35);
  @$pb.TagNumber(70)
  void clearDownloadUpdateRequest() => $_clearField(70);
  @$pb.TagNumber(70)
  DownloadUpdateRequest ensureDownloadUpdateRequest() => $_ensure(35);

  @$pb.TagNumber(71)
  DownloadProgress get downloadProgress => $_getN(36);
  @$pb.TagNumber(71)
  set downloadProgress(DownloadProgress value) => $_setField(71, value);
  @$pb.TagNumber(71)
  $core.bool hasDownloadProgress() => $_has(36);
  @$pb.TagNumber(71)
  void clearDownloadProgress() => $_clearField(71);
  @$pb.TagNumber(71)
  DownloadProgress ensureDownloadProgress() => $_ensure(36);

  @$pb.TagNumber(72)
  CancelDownloadRequest get cancelDownloadRequest => $_getN(37);
  @$pb.TagNumber(72)
  set cancelDownloadRequest(CancelDownloadRequest value) =>
      $_setField(72, value);
  @$pb.TagNumber(72)
  $core.bool hasCancelDownloadRequest() => $_has(37);
  @$pb.TagNumber(72)
  void clearCancelDownloadRequest() => $_clearField(72);
  @$pb.TagNumber(72)
  CancelDownloadRequest ensureCancelDownloadRequest() => $_ensure(37);

  @$pb.TagNumber(80)
  UiConfigRequest get uiConfigRequest => $_getN(38);
  @$pb.TagNumber(80)
  set uiConfigRequest(UiConfigRequest value) => $_setField(80, value);
  @$pb.TagNumber(80)
  $core.bool hasUiConfigRequest() => $_has(38);
  @$pb.TagNumber(80)
  void clearUiConfigRequest() => $_clearField(80);
  @$pb.TagNumber(80)
  UiConfigRequest ensureUiConfigRequest() => $_ensure(38);

  @$pb.TagNumber(81)
  UiConfigResponse get uiConfigResponse => $_getN(39);
  @$pb.TagNumber(81)
  set uiConfigResponse(UiConfigResponse value) => $_setField(81, value);
  @$pb.TagNumber(81)
  $core.bool hasUiConfigResponse() => $_has(39);
  @$pb.TagNumber(81)
  void clearUiConfigResponse() => $_clearField(81);
  @$pb.TagNumber(81)
  UiConfigResponse ensureUiConfigResponse() => $_ensure(39);

  @$pb.TagNumber(90)
  SettingsPatch get settingsPatch => $_getN(40);
  @$pb.TagNumber(90)
  set settingsPatch(SettingsPatch value) => $_setField(90, value);
  @$pb.TagNumber(90)
  $core.bool hasSettingsPatch() => $_has(40);
  @$pb.TagNumber(90)
  void clearSettingsPatch() => $_clearField(90);
  @$pb.TagNumber(90)
  SettingsPatch ensureSettingsPatch() => $_ensure(40);

  @$pb.TagNumber(99)
  Error get error => $_getN(41);
  @$pb.TagNumber(99)
  set error(Error value) => $_setField(99, value);
  @$pb.TagNumber(99)
  $core.bool hasError() => $_has(41);
  @$pb.TagNumber(99)
  void clearError() => $_clearField(99);
  @$pb.TagNumber(99)
  Error ensureError() => $_ensure(41);
}

enum SettingValue_Value {
  boolValue,
  intValue,
  doubleValue,
  stringValue,
  mapValue,
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
    StringMap? mapValue,
  }) {
    final result = SettingValue._();
    if (boolValue != null) result.boolValue = boolValue;
    if (intValue != null) result.intValue = intValue;
    if (doubleValue != null) result.doubleValue = doubleValue;
    if (stringValue != null) result.stringValue = stringValue;
    if (mapValue != null) result.mapValue = mapValue;
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
    9: SettingValue_Value.mapValue,
    0: SettingValue_Value.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingValue',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingValue.$_createMessage)
    ..oo(0, [1, 2, 3, 4, 9])
    ..aOB(1, _omitFieldNames ? '' : 'boolValue')
    ..aInt64(2, _omitFieldNames ? '' : 'intValue')
    ..aD(3, _omitFieldNames ? '' : 'doubleValue')
    ..aOS(4, _omitFieldNames ? '' : 'stringValue')
    ..aOM<StringMap>(9, _omitFieldNames ? '' : 'mapValue',
        subBuilder: StringMap.$_createMessage)
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
  @$pb.TagNumber(9)
  SettingValue_Value whichValue() => _SettingValue_ValueByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(1)
  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(9)
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

  @$pb.TagNumber(9)
  StringMap get mapValue => $_getN(4);
  @$pb.TagNumber(9)
  set mapValue(StringMap value) => $_setField(9, value);
  @$pb.TagNumber(9)
  $core.bool hasMapValue() => $_has(4);
  @$pb.TagNumber(9)
  void clearMapValue() => $_clearField(9);
  @$pb.TagNumber(9)
  StringMap ensureMapValue() => $_ensure(4);
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
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? shortcuts,
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
    if (shortcuts != null) result.shortcuts.addEntries(shortcuts);
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
    ..m<$core.String, $core.String>(37, _omitFieldNames ? '' : 'shortcuts',
        entryClassName: 'AppSettings.ShortcutsEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
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
  $pb.PbMap<$core.String, $core.String> get shortcuts => $_getMap(36);
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

class CanvasOverviewRequest extends $pb.GeneratedMessage {
  factory CanvasOverviewRequest({
    $core.int? w,
    $core.int? h,
    CanvasAction? action,
  }) {
    final result = CanvasOverviewRequest._();
    if (w != null) result.w = w;
    if (h != null) result.h = h;
    if (action != null) result.action = action;
    return result;
  }

  CanvasOverviewRequest._();

  factory CanvasOverviewRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CanvasOverviewRequest()..mergeFromBuffer(data, registry);
  factory CanvasOverviewRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CanvasOverviewRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CanvasOverviewRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: CanvasOverviewRequest.$_createMessage)
    ..aI(1, _omitFieldNames ? '' : 'w')
    ..aI(2, _omitFieldNames ? '' : 'h')
    ..aE<CanvasAction>(3, _omitFieldNames ? '' : 'action',
        enumValues: CanvasAction.values)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CanvasOverviewRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CanvasOverviewRequest copyWith(
          void Function(CanvasOverviewRequest) updates) =>
      super.copyWith((message) => updates(message as CanvasOverviewRequest))
          as CanvasOverviewRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use CanvasOverviewRequest() / CanvasOverviewRequest.new instead')
  static CanvasOverviewRequest create() => CanvasOverviewRequest._();
  static $pb.GeneratedMessage $_createMessage() => CanvasOverviewRequest._();
  @$core.override
  CanvasOverviewRequest createEmptyInstance() => CanvasOverviewRequest._();
  @$core.pragma('dart2js:noInline')
  static CanvasOverviewRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CanvasOverviewRequest>(
          CanvasOverviewRequest.$_createMessage);
  static CanvasOverviewRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get w => $_getIZ(0);
  @$pb.TagNumber(1)
  set w($core.int value) => $_setSignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasW() => $_has(0);
  @$pb.TagNumber(1)
  void clearW() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get h => $_getIZ(1);
  @$pb.TagNumber(2)
  set h($core.int value) => $_setSignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasH() => $_has(1);
  @$pb.TagNumber(2)
  void clearH() => $_clearField(2);

  @$pb.TagNumber(3)
  CanvasAction get action => $_getN(2);
  @$pb.TagNumber(3)
  set action(CanvasAction value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasAction() => $_has(2);
  @$pb.TagNumber(3)
  void clearAction() => $_clearField(3);
}

class CanvasOverviewResponse extends $pb.GeneratedMessage {
  factory CanvasOverviewResponse({
    $core.List<$core.int>? png,
    $core.Iterable<$core.double>? rect,
  }) {
    final result = CanvasOverviewResponse._();
    if (png != null) result.png = png;
    if (rect != null) result.rect.addAll(rect);
    return result;
  }

  CanvasOverviewResponse._();

  factory CanvasOverviewResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CanvasOverviewResponse()..mergeFromBuffer(data, registry);
  factory CanvasOverviewResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CanvasOverviewResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CanvasOverviewResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: CanvasOverviewResponse.$_createMessage)
    ..a<$core.List<$core.int>>(
        1, _omitFieldNames ? '' : 'png', $pb.PbFieldType.OY)
    ..p<$core.double>(2, _omitFieldNames ? '' : 'rect', $pb.PbFieldType.KD)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CanvasOverviewResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CanvasOverviewResponse copyWith(
          void Function(CanvasOverviewResponse) updates) =>
      super.copyWith((message) => updates(message as CanvasOverviewResponse))
          as CanvasOverviewResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use CanvasOverviewResponse() / CanvasOverviewResponse.new instead')
  static CanvasOverviewResponse create() => CanvasOverviewResponse._();
  static $pb.GeneratedMessage $_createMessage() => CanvasOverviewResponse._();
  @$core.override
  CanvasOverviewResponse createEmptyInstance() => CanvasOverviewResponse._();
  @$core.pragma('dart2js:noInline')
  static CanvasOverviewResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CanvasOverviewResponse>(
          CanvasOverviewResponse.$_createMessage);
  static CanvasOverviewResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.List<$core.int> get png => $_getN(0);
  @$pb.TagNumber(1)
  set png($core.List<$core.int> value) => $_setBytes(0, value);
  @$pb.TagNumber(1)
  $core.bool hasPng() => $_has(0);
  @$pb.TagNumber(1)
  void clearPng() => $_clearField(1);

  @$pb.TagNumber(2)
  $pb.PbList<$core.double> get rect => $_getList(1);
}

class BackupCommandRequest extends $pb.GeneratedMessage {
  factory BackupCommandRequest({
    BackupCommand? command,
  }) {
    final result = BackupCommandRequest._();
    if (command != null) result.command = command;
    return result;
  }

  BackupCommandRequest._();

  factory BackupCommandRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BackupCommandRequest()..mergeFromBuffer(data, registry);
  factory BackupCommandRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BackupCommandRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BackupCommandRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BackupCommandRequest.$_createMessage)
    ..aE<BackupCommand>(1, _omitFieldNames ? '' : 'command',
        enumValues: BackupCommand.values)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BackupCommandRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BackupCommandRequest copyWith(void Function(BackupCommandRequest) updates) =>
      super.copyWith((message) => updates(message as BackupCommandRequest))
          as BackupCommandRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use BackupCommandRequest() / BackupCommandRequest.new instead')
  static BackupCommandRequest create() => BackupCommandRequest._();
  static $pb.GeneratedMessage $_createMessage() => BackupCommandRequest._();
  @$core.override
  BackupCommandRequest createEmptyInstance() => BackupCommandRequest._();
  @$core.pragma('dart2js:noInline')
  static BackupCommandRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<BackupCommandRequest>(
          BackupCommandRequest.$_createMessage);
  static BackupCommandRequest? _defaultInstance;

  @$pb.TagNumber(1)
  BackupCommand get command => $_getN(0);
  @$pb.TagNumber(1)
  set command(BackupCommand value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasCommand() => $_has(0);
  @$pb.TagNumber(1)
  void clearCommand() => $_clearField(1);
}

class BackupOutcome extends $pb.GeneratedMessage {
  factory BackupOutcome({
    $core.bool? ok,
    $core.String? message,
    $core.String? kind,
  }) {
    final result = BackupOutcome._();
    if (ok != null) result.ok = ok;
    if (message != null) result.message = message;
    if (kind != null) result.kind = kind;
    return result;
  }

  BackupOutcome._();

  factory BackupOutcome.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BackupOutcome()..mergeFromBuffer(data, registry);
  factory BackupOutcome.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      BackupOutcome()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'BackupOutcome',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: BackupOutcome.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..aOS(2, _omitFieldNames ? '' : 'message')
    ..aOS(3, _omitFieldNames ? '' : 'kind')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BackupOutcome clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  BackupOutcome copyWith(void Function(BackupOutcome) updates) =>
      super.copyWith((message) => updates(message as BackupOutcome))
          as BackupOutcome;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use BackupOutcome() / BackupOutcome.new instead')
  static BackupOutcome create() => BackupOutcome._();
  static $pb.GeneratedMessage $_createMessage() => BackupOutcome._();
  @$core.override
  BackupOutcome createEmptyInstance() => BackupOutcome._();
  @$core.pragma('dart2js:noInline')
  static BackupOutcome getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<BackupOutcome>(
          BackupOutcome.$_createMessage);
  static BackupOutcome? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get message => $_getSZ(1);
  @$pb.TagNumber(2)
  set message($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMessage() => $_has(1);
  @$pb.TagNumber(2)
  void clearMessage() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get kind => $_getSZ(2);
  @$pb.TagNumber(3)
  set kind($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasKind() => $_has(2);
  @$pb.TagNumber(3)
  void clearKind() => $_clearField(3);
}

class ChatLoginRequest extends $pb.GeneratedMessage {
  factory ChatLoginRequest() => ChatLoginRequest._();

  ChatLoginRequest._();

  factory ChatLoginRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ChatLoginRequest()..mergeFromBuffer(data, registry);
  factory ChatLoginRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ChatLoginRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ChatLoginRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ChatLoginRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChatLoginRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChatLoginRequest copyWith(void Function(ChatLoginRequest) updates) =>
      super.copyWith((message) => updates(message as ChatLoginRequest))
          as ChatLoginRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ChatLoginRequest() / ChatLoginRequest.new instead')
  static ChatLoginRequest create() => ChatLoginRequest._();
  static $pb.GeneratedMessage $_createMessage() => ChatLoginRequest._();
  @$core.override
  ChatLoginRequest createEmptyInstance() => ChatLoginRequest._();
  @$core.pragma('dart2js:noInline')
  static ChatLoginRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ChatLoginRequest>(
          ChatLoginRequest.$_createMessage);
  static ChatLoginRequest? _defaultInstance;
}

class ChatLoginOutcome extends $pb.GeneratedMessage {
  factory ChatLoginOutcome({
    $core.bool? ok,
    $core.String? message,
    $core.String? kind,
  }) {
    final result = ChatLoginOutcome._();
    if (ok != null) result.ok = ok;
    if (message != null) result.message = message;
    if (kind != null) result.kind = kind;
    return result;
  }

  ChatLoginOutcome._();

  factory ChatLoginOutcome.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ChatLoginOutcome()..mergeFromBuffer(data, registry);
  factory ChatLoginOutcome.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ChatLoginOutcome()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ChatLoginOutcome',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ChatLoginOutcome.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..aOS(2, _omitFieldNames ? '' : 'message')
    ..aOS(3, _omitFieldNames ? '' : 'kind')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChatLoginOutcome clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChatLoginOutcome copyWith(void Function(ChatLoginOutcome) updates) =>
      super.copyWith((message) => updates(message as ChatLoginOutcome))
          as ChatLoginOutcome;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ChatLoginOutcome() / ChatLoginOutcome.new instead')
  static ChatLoginOutcome create() => ChatLoginOutcome._();
  static $pb.GeneratedMessage $_createMessage() => ChatLoginOutcome._();
  @$core.override
  ChatLoginOutcome createEmptyInstance() => ChatLoginOutcome._();
  @$core.pragma('dart2js:noInline')
  static ChatLoginOutcome getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ChatLoginOutcome>(
          ChatLoginOutcome.$_createMessage);
  static ChatLoginOutcome? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get message => $_getSZ(1);
  @$pb.TagNumber(2)
  set message($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMessage() => $_has(1);
  @$pb.TagNumber(2)
  void clearMessage() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get kind => $_getSZ(2);
  @$pb.TagNumber(3)
  set kind($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasKind() => $_has(2);
  @$pb.TagNumber(3)
  void clearKind() => $_clearField(3);
}

class ExportPdfRequest extends $pb.GeneratedMessage {
  factory ExportPdfRequest() => ExportPdfRequest._();

  ExportPdfRequest._();

  factory ExportPdfRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ExportPdfRequest()..mergeFromBuffer(data, registry);
  factory ExportPdfRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ExportPdfRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ExportPdfRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ExportPdfRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ExportPdfRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ExportPdfRequest copyWith(void Function(ExportPdfRequest) updates) =>
      super.copyWith((message) => updates(message as ExportPdfRequest))
          as ExportPdfRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ExportPdfRequest() / ExportPdfRequest.new instead')
  static ExportPdfRequest create() => ExportPdfRequest._();
  static $pb.GeneratedMessage $_createMessage() => ExportPdfRequest._();
  @$core.override
  ExportPdfRequest createEmptyInstance() => ExportPdfRequest._();
  @$core.pragma('dart2js:noInline')
  static ExportPdfRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<ExportPdfRequest>(
          ExportPdfRequest.$_createMessage);
  static ExportPdfRequest? _defaultInstance;
}

class CheckUpdateRequest extends $pb.GeneratedMessage {
  factory CheckUpdateRequest() => CheckUpdateRequest._();

  CheckUpdateRequest._();

  factory CheckUpdateRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CheckUpdateRequest()..mergeFromBuffer(data, registry);
  factory CheckUpdateRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CheckUpdateRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CheckUpdateRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: CheckUpdateRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CheckUpdateRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CheckUpdateRequest copyWith(void Function(CheckUpdateRequest) updates) =>
      super.copyWith((message) => updates(message as CheckUpdateRequest))
          as CheckUpdateRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use CheckUpdateRequest() / CheckUpdateRequest.new instead')
  static CheckUpdateRequest create() => CheckUpdateRequest._();
  static $pb.GeneratedMessage $_createMessage() => CheckUpdateRequest._();
  @$core.override
  CheckUpdateRequest createEmptyInstance() => CheckUpdateRequest._();
  @$core.pragma('dart2js:noInline')
  static CheckUpdateRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CheckUpdateRequest>(
          CheckUpdateRequest.$_createMessage);
  static CheckUpdateRequest? _defaultInstance;
}

class UpdateInfo extends $pb.GeneratedMessage {
  factory UpdateInfo({
    $core.bool? ok,
    $core.String? current,
    $core.String? latest,
    $core.bool? hasUpdate,
    $core.String? url,
    $core.String? notes,
    $core.String? error,
  }) {
    final result = UpdateInfo._();
    if (ok != null) result.ok = ok;
    if (current != null) result.current = current;
    if (latest != null) result.latest = latest;
    if (hasUpdate != null) result.hasUpdate = hasUpdate;
    if (url != null) result.url = url;
    if (notes != null) result.notes = notes;
    if (error != null) result.error = error;
    return result;
  }

  UpdateInfo._();

  factory UpdateInfo.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UpdateInfo()..mergeFromBuffer(data, registry);
  factory UpdateInfo.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UpdateInfo()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UpdateInfo',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UpdateInfo.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..aOS(2, _omitFieldNames ? '' : 'current')
    ..aOS(3, _omitFieldNames ? '' : 'latest')
    ..aOB(4, _omitFieldNames ? '' : 'hasUpdate')
    ..aOS(5, _omitFieldNames ? '' : 'url')
    ..aOS(6, _omitFieldNames ? '' : 'notes')
    ..aOS(7, _omitFieldNames ? '' : 'error')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UpdateInfo clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UpdateInfo copyWith(void Function(UpdateInfo) updates) =>
      super.copyWith((message) => updates(message as UpdateInfo)) as UpdateInfo;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UpdateInfo() / UpdateInfo.new instead')
  static UpdateInfo create() => UpdateInfo._();
  static $pb.GeneratedMessage $_createMessage() => UpdateInfo._();
  @$core.override
  UpdateInfo createEmptyInstance() => UpdateInfo._();
  @$core.pragma('dart2js:noInline')
  static UpdateInfo getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UpdateInfo>(UpdateInfo.$_createMessage);
  static UpdateInfo? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get current => $_getSZ(1);
  @$pb.TagNumber(2)
  set current($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasCurrent() => $_has(1);
  @$pb.TagNumber(2)
  void clearCurrent() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get latest => $_getSZ(2);
  @$pb.TagNumber(3)
  set latest($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasLatest() => $_has(2);
  @$pb.TagNumber(3)
  void clearLatest() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get hasUpdate => $_getBF(3);
  @$pb.TagNumber(4)
  set hasUpdate($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasHasUpdate() => $_has(3);
  @$pb.TagNumber(4)
  void clearHasUpdate() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get url => $_getSZ(4);
  @$pb.TagNumber(5)
  set url($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasUrl() => $_has(4);
  @$pb.TagNumber(5)
  void clearUrl() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get notes => $_getSZ(5);
  @$pb.TagNumber(6)
  set notes($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasNotes() => $_has(5);
  @$pb.TagNumber(6)
  void clearNotes() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get error => $_getSZ(6);
  @$pb.TagNumber(7)
  set error($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasError() => $_has(6);
  @$pb.TagNumber(7)
  void clearError() => $_clearField(7);
}

class NasBackupRequest extends $pb.GeneratedMessage {
  factory NasBackupRequest() => NasBackupRequest._();

  NasBackupRequest._();

  factory NasBackupRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NasBackupRequest()..mergeFromBuffer(data, registry);
  factory NasBackupRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NasBackupRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'NasBackupRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: NasBackupRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NasBackupRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NasBackupRequest copyWith(void Function(NasBackupRequest) updates) =>
      super.copyWith((message) => updates(message as NasBackupRequest))
          as NasBackupRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use NasBackupRequest() / NasBackupRequest.new instead')
  static NasBackupRequest create() => NasBackupRequest._();
  static $pb.GeneratedMessage $_createMessage() => NasBackupRequest._();
  @$core.override
  NasBackupRequest createEmptyInstance() => NasBackupRequest._();
  @$core.pragma('dart2js:noInline')
  static NasBackupRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<NasBackupRequest>(
          NasBackupRequest.$_createMessage);
  static NasBackupRequest? _defaultInstance;
}

class NasBackupOutcome extends $pb.GeneratedMessage {
  factory NasBackupOutcome({
    $core.bool? ok,
    $fixnum.Int64? sizeBytes,
    $fixnum.Int64? strokeCount,
    $core.String? error,
    $core.String? hint,
  }) {
    final result = NasBackupOutcome._();
    if (ok != null) result.ok = ok;
    if (sizeBytes != null) result.sizeBytes = sizeBytes;
    if (strokeCount != null) result.strokeCount = strokeCount;
    if (error != null) result.error = error;
    if (hint != null) result.hint = hint;
    return result;
  }

  NasBackupOutcome._();

  factory NasBackupOutcome.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NasBackupOutcome()..mergeFromBuffer(data, registry);
  factory NasBackupOutcome.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      NasBackupOutcome()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'NasBackupOutcome',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: NasBackupOutcome.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..a<$fixnum.Int64>(
        2, _omitFieldNames ? '' : 'sizeBytes', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        3, _omitFieldNames ? '' : 'strokeCount', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aOS(4, _omitFieldNames ? '' : 'error')
    ..aOS(5, _omitFieldNames ? '' : 'hint')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NasBackupOutcome clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  NasBackupOutcome copyWith(void Function(NasBackupOutcome) updates) =>
      super.copyWith((message) => updates(message as NasBackupOutcome))
          as NasBackupOutcome;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use NasBackupOutcome() / NasBackupOutcome.new instead')
  static NasBackupOutcome create() => NasBackupOutcome._();
  static $pb.GeneratedMessage $_createMessage() => NasBackupOutcome._();
  @$core.override
  NasBackupOutcome createEmptyInstance() => NasBackupOutcome._();
  @$core.pragma('dart2js:noInline')
  static NasBackupOutcome getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<NasBackupOutcome>(
          NasBackupOutcome.$_createMessage);
  static NasBackupOutcome? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get sizeBytes => $_getI64(1);
  @$pb.TagNumber(2)
  set sizeBytes($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSizeBytes() => $_has(1);
  @$pb.TagNumber(2)
  void clearSizeBytes() => $_clearField(2);

  @$pb.TagNumber(3)
  $fixnum.Int64 get strokeCount => $_getI64(2);
  @$pb.TagNumber(3)
  set strokeCount($fixnum.Int64 value) => $_setInt64(2, value);
  @$pb.TagNumber(3)
  $core.bool hasStrokeCount() => $_has(2);
  @$pb.TagNumber(3)
  void clearStrokeCount() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get error => $_getSZ(3);
  @$pb.TagNumber(4)
  set error($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasError() => $_has(3);
  @$pb.TagNumber(4)
  void clearError() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get hint => $_getSZ(4);
  @$pb.TagNumber(5)
  set hint($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasHint() => $_has(4);
  @$pb.TagNumber(5)
  void clearHint() => $_clearField(5);
}

class ApplyUpdateRequest extends $pb.GeneratedMessage {
  factory ApplyUpdateRequest() => ApplyUpdateRequest._();

  ApplyUpdateRequest._();

  factory ApplyUpdateRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ApplyUpdateRequest()..mergeFromBuffer(data, registry);
  factory ApplyUpdateRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ApplyUpdateRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ApplyUpdateRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ApplyUpdateRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApplyUpdateRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApplyUpdateRequest copyWith(void Function(ApplyUpdateRequest) updates) =>
      super.copyWith((message) => updates(message as ApplyUpdateRequest))
          as ApplyUpdateRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ApplyUpdateRequest() / ApplyUpdateRequest.new instead')
  static ApplyUpdateRequest create() => ApplyUpdateRequest._();
  static $pb.GeneratedMessage $_createMessage() => ApplyUpdateRequest._();
  @$core.override
  ApplyUpdateRequest createEmptyInstance() => ApplyUpdateRequest._();
  @$core.pragma('dart2js:noInline')
  static ApplyUpdateRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ApplyUpdateRequest>(
          ApplyUpdateRequest.$_createMessage);
  static ApplyUpdateRequest? _defaultInstance;
}

class ApplyUpdateOutcome extends $pb.GeneratedMessage {
  factory ApplyUpdateOutcome({
    $core.bool? ok,
    $core.String? message,
  }) {
    final result = ApplyUpdateOutcome._();
    if (ok != null) result.ok = ok;
    if (message != null) result.message = message;
    return result;
  }

  ApplyUpdateOutcome._();

  factory ApplyUpdateOutcome.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ApplyUpdateOutcome()..mergeFromBuffer(data, registry);
  factory ApplyUpdateOutcome.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      ApplyUpdateOutcome()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ApplyUpdateOutcome',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: ApplyUpdateOutcome.$_createMessage)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..aOS(2, _omitFieldNames ? '' : 'message')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApplyUpdateOutcome clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApplyUpdateOutcome copyWith(void Function(ApplyUpdateOutcome) updates) =>
      super.copyWith((message) => updates(message as ApplyUpdateOutcome))
          as ApplyUpdateOutcome;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use ApplyUpdateOutcome() / ApplyUpdateOutcome.new instead')
  static ApplyUpdateOutcome create() => ApplyUpdateOutcome._();
  static $pb.GeneratedMessage $_createMessage() => ApplyUpdateOutcome._();
  @$core.override
  ApplyUpdateOutcome createEmptyInstance() => ApplyUpdateOutcome._();
  @$core.pragma('dart2js:noInline')
  static ApplyUpdateOutcome getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ApplyUpdateOutcome>(
          ApplyUpdateOutcome.$_createMessage);
  static ApplyUpdateOutcome? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get message => $_getSZ(1);
  @$pb.TagNumber(2)
  set message($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMessage() => $_has(1);
  @$pb.TagNumber(2)
  void clearMessage() => $_clearField(2);
}

class DownloadUpdateRequest extends $pb.GeneratedMessage {
  factory DownloadUpdateRequest() => DownloadUpdateRequest._();

  DownloadUpdateRequest._();

  factory DownloadUpdateRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DownloadUpdateRequest()..mergeFromBuffer(data, registry);
  factory DownloadUpdateRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DownloadUpdateRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'DownloadUpdateRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: DownloadUpdateRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DownloadUpdateRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DownloadUpdateRequest copyWith(
          void Function(DownloadUpdateRequest) updates) =>
      super.copyWith((message) => updates(message as DownloadUpdateRequest))
          as DownloadUpdateRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use DownloadUpdateRequest() / DownloadUpdateRequest.new instead')
  static DownloadUpdateRequest create() => DownloadUpdateRequest._();
  static $pb.GeneratedMessage $_createMessage() => DownloadUpdateRequest._();
  @$core.override
  DownloadUpdateRequest createEmptyInstance() => DownloadUpdateRequest._();
  @$core.pragma('dart2js:noInline')
  static DownloadUpdateRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<DownloadUpdateRequest>(
          DownloadUpdateRequest.$_createMessage);
  static DownloadUpdateRequest? _defaultInstance;
}

class DownloadProgress extends $pb.GeneratedMessage {
  factory DownloadProgress({
    $fixnum.Int64? received,
    $fixnum.Int64? total,
    $core.bool? done,
    $core.String? error,
    $core.String? path,
  }) {
    final result = DownloadProgress._();
    if (received != null) result.received = received;
    if (total != null) result.total = total;
    if (done != null) result.done = done;
    if (error != null) result.error = error;
    if (path != null) result.path = path;
    return result;
  }

  DownloadProgress._();

  factory DownloadProgress.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DownloadProgress()..mergeFromBuffer(data, registry);
  factory DownloadProgress.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      DownloadProgress()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'DownloadProgress',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: DownloadProgress.$_createMessage)
    ..a<$fixnum.Int64>(
        1, _omitFieldNames ? '' : 'received', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(2, _omitFieldNames ? '' : 'total', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aOB(3, _omitFieldNames ? '' : 'done')
    ..aOS(4, _omitFieldNames ? '' : 'error')
    ..aOS(5, _omitFieldNames ? '' : 'path')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DownloadProgress clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  DownloadProgress copyWith(void Function(DownloadProgress) updates) =>
      super.copyWith((message) => updates(message as DownloadProgress))
          as DownloadProgress;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use DownloadProgress() / DownloadProgress.new instead')
  static DownloadProgress create() => DownloadProgress._();
  static $pb.GeneratedMessage $_createMessage() => DownloadProgress._();
  @$core.override
  DownloadProgress createEmptyInstance() => DownloadProgress._();
  @$core.pragma('dart2js:noInline')
  static DownloadProgress getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<DownloadProgress>(
          DownloadProgress.$_createMessage);
  static DownloadProgress? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get received => $_getI64(0);
  @$pb.TagNumber(1)
  set received($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasReceived() => $_has(0);
  @$pb.TagNumber(1)
  void clearReceived() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get total => $_getI64(1);
  @$pb.TagNumber(2)
  set total($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTotal() => $_has(1);
  @$pb.TagNumber(2)
  void clearTotal() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get done => $_getBF(2);
  @$pb.TagNumber(3)
  set done($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasDone() => $_has(2);
  @$pb.TagNumber(3)
  void clearDone() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get error => $_getSZ(3);
  @$pb.TagNumber(4)
  set error($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasError() => $_has(3);
  @$pb.TagNumber(4)
  void clearError() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get path => $_getSZ(4);
  @$pb.TagNumber(5)
  set path($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasPath() => $_has(4);
  @$pb.TagNumber(5)
  void clearPath() => $_clearField(5);
}

class CancelDownloadRequest extends $pb.GeneratedMessage {
  factory CancelDownloadRequest() => CancelDownloadRequest._();

  CancelDownloadRequest._();

  factory CancelDownloadRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CancelDownloadRequest()..mergeFromBuffer(data, registry);
  factory CancelDownloadRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      CancelDownloadRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CancelDownloadRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: CancelDownloadRequest.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CancelDownloadRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CancelDownloadRequest copyWith(
          void Function(CancelDownloadRequest) updates) =>
      super.copyWith((message) => updates(message as CancelDownloadRequest))
          as CancelDownloadRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated(
      'Use CancelDownloadRequest() / CancelDownloadRequest.new instead')
  static CancelDownloadRequest create() => CancelDownloadRequest._();
  static $pb.GeneratedMessage $_createMessage() => CancelDownloadRequest._();
  @$core.override
  CancelDownloadRequest createEmptyInstance() => CancelDownloadRequest._();
  @$core.pragma('dart2js:noInline')
  static CancelDownloadRequest getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CancelDownloadRequest>(
          CancelDownloadRequest.$_createMessage);
  static CancelDownloadRequest? _defaultInstance;
}

class UiConfigRequest extends $pb.GeneratedMessage {
  factory UiConfigRequest({
    $core.int? panelProtocolVersion,
  }) {
    final result = UiConfigRequest._();
    if (panelProtocolVersion != null)
      result.panelProtocolVersion = panelProtocolVersion;
    return result;
  }

  UiConfigRequest._();

  factory UiConfigRequest.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfigRequest()..mergeFromBuffer(data, registry);
  factory UiConfigRequest.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfigRequest()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiConfigRequest',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiConfigRequest.$_createMessage)
    ..aI(1, _omitFieldNames ? '' : 'panelProtocolVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfigRequest clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfigRequest copyWith(void Function(UiConfigRequest) updates) =>
      super.copyWith((message) => updates(message as UiConfigRequest))
          as UiConfigRequest;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiConfigRequest() / UiConfigRequest.new instead')
  static UiConfigRequest create() => UiConfigRequest._();
  static $pb.GeneratedMessage $_createMessage() => UiConfigRequest._();
  @$core.override
  UiConfigRequest createEmptyInstance() => UiConfigRequest._();
  @$core.pragma('dart2js:noInline')
  static UiConfigRequest getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiConfigRequest>(
          UiConfigRequest.$_createMessage);
  static UiConfigRequest? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get panelProtocolVersion => $_getIZ(0);
  @$pb.TagNumber(1)
  set panelProtocolVersion($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasPanelProtocolVersion() => $_has(0);
  @$pb.TagNumber(1)
  void clearPanelProtocolVersion() => $_clearField(1);
}

class UiConfigResponse extends $pb.GeneratedMessage {
  factory UiConfigResponse({
    UiConfig? config,
  }) {
    final result = UiConfigResponse._();
    if (config != null) result.config = config;
    return result;
  }

  UiConfigResponse._();

  factory UiConfigResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfigResponse()..mergeFromBuffer(data, registry);
  factory UiConfigResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfigResponse()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiConfigResponse',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiConfigResponse.$_createMessage)
    ..aOM<UiConfig>(1, _omitFieldNames ? '' : 'config',
        subBuilder: UiConfig.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfigResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfigResponse copyWith(void Function(UiConfigResponse) updates) =>
      super.copyWith((message) => updates(message as UiConfigResponse))
          as UiConfigResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiConfigResponse() / UiConfigResponse.new instead')
  static UiConfigResponse create() => UiConfigResponse._();
  static $pb.GeneratedMessage $_createMessage() => UiConfigResponse._();
  @$core.override
  UiConfigResponse createEmptyInstance() => UiConfigResponse._();
  @$core.pragma('dart2js:noInline')
  static UiConfigResponse getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiConfigResponse>(
          UiConfigResponse.$_createMessage);
  static UiConfigResponse? _defaultInstance;

  @$pb.TagNumber(1)
  UiConfig get config => $_getN(0);
  @$pb.TagNumber(1)
  set config(UiConfig value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasConfig() => $_has(0);
  @$pb.TagNumber(1)
  void clearConfig() => $_clearField(1);
  @$pb.TagNumber(1)
  UiConfig ensureConfig() => $_ensure(0);
}

class UiConfig extends $pb.GeneratedMessage {
  factory UiConfig({
    $core.String? platform,
    $core.bool? isWindows,
    $core.double? fontScale,
    $core.String? chordSeparator,
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? chordNames,
    $core.String? modifierWord,
    $core.Iterable<$core.MapEntry<$core.String, LocalizedText>>? messages,
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? defaults,
    $core.Iterable<$core.MapEntry<$core.String, $core.String>>? holdHotkeys,
    $core.Iterable<$core.String>? reserved,
    $core.Iterable<$core.String>? forceBound,
    $core.Iterable<$core.String>? actionOrder,
    UiModifierRow? modifierRow,
    $core.Iterable<$core.MapEntry<$core.String, $core.bool>>? features,
    $core.Iterable<UiSettingSpec>? settings,
    $core.Iterable<UiFormSection>? settingsSections,
    $core.Iterable<UiCustomBlock>? settingsBlocks,
    $core.int? protocolVersion,
  }) {
    final result = UiConfig._();
    if (platform != null) result.platform = platform;
    if (isWindows != null) result.isWindows = isWindows;
    if (fontScale != null) result.fontScale = fontScale;
    if (chordSeparator != null) result.chordSeparator = chordSeparator;
    if (chordNames != null) result.chordNames.addEntries(chordNames);
    if (modifierWord != null) result.modifierWord = modifierWord;
    if (messages != null) result.messages.addEntries(messages);
    if (defaults != null) result.defaults.addEntries(defaults);
    if (holdHotkeys != null) result.holdHotkeys.addEntries(holdHotkeys);
    if (reserved != null) result.reserved.addAll(reserved);
    if (forceBound != null) result.forceBound.addAll(forceBound);
    if (actionOrder != null) result.actionOrder.addAll(actionOrder);
    if (modifierRow != null) result.modifierRow = modifierRow;
    if (features != null) result.features.addEntries(features);
    if (settings != null) result.settings.addAll(settings);
    if (settingsSections != null)
      result.settingsSections.addAll(settingsSections);
    if (settingsBlocks != null) result.settingsBlocks.addAll(settingsBlocks);
    if (protocolVersion != null) result.protocolVersion = protocolVersion;
    return result;
  }

  UiConfig._();

  factory UiConfig.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfig()..mergeFromBuffer(data, registry);
  factory UiConfig.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiConfig()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiConfig',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiConfig.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'platform')
    ..aOB(2, _omitFieldNames ? '' : 'isWindows')
    ..aD(3, _omitFieldNames ? '' : 'fontScale')
    ..aOS(4, _omitFieldNames ? '' : 'chordSeparator')
    ..m<$core.String, $core.String>(5, _omitFieldNames ? '' : 'chordNames',
        entryClassName: 'UiConfig.ChordNamesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..aOS(6, _omitFieldNames ? '' : 'modifierWord')
    ..m<$core.String, LocalizedText>(7, _omitFieldNames ? '' : 'messages',
        entryClassName: 'UiConfig.MessagesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OM,
        valueCreator: LocalizedText.$_createMessage,
        valueDefaultOrMaker: LocalizedText.getDefault,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..m<$core.String, $core.String>(8, _omitFieldNames ? '' : 'defaults',
        entryClassName: 'UiConfig.DefaultsEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..m<$core.String, $core.String>(9, _omitFieldNames ? '' : 'holdHotkeys',
        entryClassName: 'UiConfig.HoldHotkeysEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OS,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..pPS(10, _omitFieldNames ? '' : 'reserved')
    ..pPS(11, _omitFieldNames ? '' : 'forceBound')
    ..pPS(12, _omitFieldNames ? '' : 'actionOrder')
    ..aOM<UiModifierRow>(13, _omitFieldNames ? '' : 'modifierRow',
        subBuilder: UiModifierRow.$_createMessage)
    ..m<$core.String, $core.bool>(14, _omitFieldNames ? '' : 'features',
        entryClassName: 'UiConfig.FeaturesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OB,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..pPM<UiSettingSpec>(15, _omitFieldNames ? '' : 'settings',
        subBuilder: UiSettingSpec.$_createMessage)
    ..pPM<UiFormSection>(16, _omitFieldNames ? '' : 'settingsSections',
        subBuilder: UiFormSection.$_createMessage)
    ..pPM<UiCustomBlock>(17, _omitFieldNames ? '' : 'settingsBlocks',
        subBuilder: UiCustomBlock.$_createMessage)
    ..aI(18, _omitFieldNames ? '' : 'protocolVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfig clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiConfig copyWith(void Function(UiConfig) updates) =>
      super.copyWith((message) => updates(message as UiConfig)) as UiConfig;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiConfig() / UiConfig.new instead')
  static UiConfig create() => UiConfig._();
  static $pb.GeneratedMessage $_createMessage() => UiConfig._();
  @$core.override
  UiConfig createEmptyInstance() => UiConfig._();
  @$core.pragma('dart2js:noInline')
  static UiConfig getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UiConfig>(UiConfig.$_createMessage);
  static UiConfig? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get platform => $_getSZ(0);
  @$pb.TagNumber(1)
  set platform($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasPlatform() => $_has(0);
  @$pb.TagNumber(1)
  void clearPlatform() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.bool get isWindows => $_getBF(1);
  @$pb.TagNumber(2)
  set isWindows($core.bool value) => $_setBool(1, value);
  @$pb.TagNumber(2)
  $core.bool hasIsWindows() => $_has(1);
  @$pb.TagNumber(2)
  void clearIsWindows() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.double get fontScale => $_getN(2);
  @$pb.TagNumber(3)
  set fontScale($core.double value) => $_setDouble(2, value);
  @$pb.TagNumber(3)
  $core.bool hasFontScale() => $_has(2);
  @$pb.TagNumber(3)
  void clearFontScale() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get chordSeparator => $_getSZ(3);
  @$pb.TagNumber(4)
  set chordSeparator($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasChordSeparator() => $_has(3);
  @$pb.TagNumber(4)
  void clearChordSeparator() => $_clearField(4);

  @$pb.TagNumber(5)
  $pb.PbMap<$core.String, $core.String> get chordNames => $_getMap(4);

  @$pb.TagNumber(6)
  $core.String get modifierWord => $_getSZ(5);
  @$pb.TagNumber(6)
  set modifierWord($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasModifierWord() => $_has(5);
  @$pb.TagNumber(6)
  void clearModifierWord() => $_clearField(6);

  @$pb.TagNumber(7)
  $pb.PbMap<$core.String, LocalizedText> get messages => $_getMap(6);

  @$pb.TagNumber(8)
  $pb.PbMap<$core.String, $core.String> get defaults => $_getMap(7);

  @$pb.TagNumber(9)
  $pb.PbMap<$core.String, $core.String> get holdHotkeys => $_getMap(8);

  @$pb.TagNumber(10)
  $pb.PbList<$core.String> get reserved => $_getList(9);

  @$pb.TagNumber(11)
  $pb.PbList<$core.String> get forceBound => $_getList(10);

  @$pb.TagNumber(12)
  $pb.PbList<$core.String> get actionOrder => $_getList(11);

  @$pb.TagNumber(13)
  UiModifierRow get modifierRow => $_getN(12);
  @$pb.TagNumber(13)
  set modifierRow(UiModifierRow value) => $_setField(13, value);
  @$pb.TagNumber(13)
  $core.bool hasModifierRow() => $_has(12);
  @$pb.TagNumber(13)
  void clearModifierRow() => $_clearField(13);
  @$pb.TagNumber(13)
  UiModifierRow ensureModifierRow() => $_ensure(12);

  @$pb.TagNumber(14)
  $pb.PbMap<$core.String, $core.bool> get features => $_getMap(13);

  @$pb.TagNumber(15)
  $pb.PbList<UiSettingSpec> get settings => $_getList(14);

  @$pb.TagNumber(16)
  $pb.PbList<UiFormSection> get settingsSections => $_getList(15);

  @$pb.TagNumber(17)
  $pb.PbList<UiCustomBlock> get settingsBlocks => $_getList(16);

  @$pb.TagNumber(18)
  $core.int get protocolVersion => $_getIZ(17);
  @$pb.TagNumber(18)
  set protocolVersion($core.int value) => $_setUnsignedInt32(17, value);
  @$pb.TagNumber(18)
  $core.bool hasProtocolVersion() => $_has(17);
  @$pb.TagNumber(18)
  void clearProtocolVersion() => $_clearField(18);
}

class LocalizedText extends $pb.GeneratedMessage {
  factory LocalizedText({
    $core.String? zh,
    $core.String? en,
  }) {
    final result = LocalizedText._();
    if (zh != null) result.zh = zh;
    if (en != null) result.en = en;
    return result;
  }

  LocalizedText._();

  factory LocalizedText.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      LocalizedText()..mergeFromBuffer(data, registry);
  factory LocalizedText.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      LocalizedText()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'LocalizedText',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: LocalizedText.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'zh')
    ..aOS(2, _omitFieldNames ? '' : 'en')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  LocalizedText clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  LocalizedText copyWith(void Function(LocalizedText) updates) =>
      super.copyWith((message) => updates(message as LocalizedText))
          as LocalizedText;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use LocalizedText() / LocalizedText.new instead')
  static LocalizedText create() => LocalizedText._();
  static $pb.GeneratedMessage $_createMessage() => LocalizedText._();
  @$core.override
  LocalizedText createEmptyInstance() => LocalizedText._();
  @$core.pragma('dart2js:noInline')
  static LocalizedText getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<LocalizedText>(
          LocalizedText.$_createMessage);
  static LocalizedText? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get zh => $_getSZ(0);
  @$pb.TagNumber(1)
  set zh($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasZh() => $_has(0);
  @$pb.TagNumber(1)
  void clearZh() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get en => $_getSZ(1);
  @$pb.TagNumber(2)
  set en($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasEn() => $_has(1);
  @$pb.TagNumber(2)
  void clearEn() => $_clearField(2);
}

class UiKeyCap extends $pb.GeneratedMessage {
  factory UiKeyCap({
    $core.String? name,
    $core.String? label,
    $core.double? width,
    $core.bool? modifier,
  }) {
    final result = UiKeyCap._();
    if (name != null) result.name = name;
    if (label != null) result.label = label;
    if (width != null) result.width = width;
    if (modifier != null) result.modifier = modifier;
    return result;
  }

  UiKeyCap._();

  factory UiKeyCap.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiKeyCap()..mergeFromBuffer(data, registry);
  factory UiKeyCap.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiKeyCap()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiKeyCap',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiKeyCap.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'name')
    ..aOS(2, _omitFieldNames ? '' : 'label')
    ..aD(3, _omitFieldNames ? '' : 'width')
    ..aOB(4, _omitFieldNames ? '' : 'modifier')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiKeyCap clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiKeyCap copyWith(void Function(UiKeyCap) updates) =>
      super.copyWith((message) => updates(message as UiKeyCap)) as UiKeyCap;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiKeyCap() / UiKeyCap.new instead')
  static UiKeyCap create() => UiKeyCap._();
  static $pb.GeneratedMessage $_createMessage() => UiKeyCap._();
  @$core.override
  UiKeyCap createEmptyInstance() => UiKeyCap._();
  @$core.pragma('dart2js:noInline')
  static UiKeyCap getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UiKeyCap>(UiKeyCap.$_createMessage);
  static UiKeyCap? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get name => $_getSZ(0);
  @$pb.TagNumber(1)
  set name($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasName() => $_has(0);
  @$pb.TagNumber(1)
  void clearName() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get label => $_getSZ(1);
  @$pb.TagNumber(2)
  set label($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasLabel() => $_has(1);
  @$pb.TagNumber(2)
  void clearLabel() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.double get width => $_getN(2);
  @$pb.TagNumber(3)
  set width($core.double value) => $_setDouble(2, value);
  @$pb.TagNumber(3)
  $core.bool hasWidth() => $_has(2);
  @$pb.TagNumber(3)
  void clearWidth() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get modifier => $_getBF(3);
  @$pb.TagNumber(4)
  set modifier($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasModifier() => $_has(3);
  @$pb.TagNumber(4)
  void clearModifier() => $_clearField(4);
}

class UiModifierRow extends $pb.GeneratedMessage {
  factory UiModifierRow({
    $core.Iterable<UiKeyCap>? left,
    $core.Iterable<UiKeyCap>? space,
    $core.Iterable<UiKeyCap>? right,
  }) {
    final result = UiModifierRow._();
    if (left != null) result.left.addAll(left);
    if (space != null) result.space.addAll(space);
    if (right != null) result.right.addAll(right);
    return result;
  }

  UiModifierRow._();

  factory UiModifierRow.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiModifierRow()..mergeFromBuffer(data, registry);
  factory UiModifierRow.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiModifierRow()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiModifierRow',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiModifierRow.$_createMessage)
    ..pPM<UiKeyCap>(1, _omitFieldNames ? '' : 'left',
        subBuilder: UiKeyCap.$_createMessage)
    ..pPM<UiKeyCap>(2, _omitFieldNames ? '' : 'space',
        subBuilder: UiKeyCap.$_createMessage)
    ..pPM<UiKeyCap>(3, _omitFieldNames ? '' : 'right',
        subBuilder: UiKeyCap.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiModifierRow clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiModifierRow copyWith(void Function(UiModifierRow) updates) =>
      super.copyWith((message) => updates(message as UiModifierRow))
          as UiModifierRow;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiModifierRow() / UiModifierRow.new instead')
  static UiModifierRow create() => UiModifierRow._();
  static $pb.GeneratedMessage $_createMessage() => UiModifierRow._();
  @$core.override
  UiModifierRow createEmptyInstance() => UiModifierRow._();
  @$core.pragma('dart2js:noInline')
  static UiModifierRow getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiModifierRow>(
          UiModifierRow.$_createMessage);
  static UiModifierRow? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<UiKeyCap> get left => $_getList(0);

  @$pb.TagNumber(2)
  $pb.PbList<UiKeyCap> get space => $_getList(1);

  @$pb.TagNumber(3)
  $pb.PbList<UiKeyCap> get right => $_getList(2);
}

class UiSettingSpec extends $pb.GeneratedMessage {
  factory UiSettingSpec({
    $core.String? key,
    $core.String? dbKey,
    $core.String? kind,
    $core.bool? mac,
    $core.bool? win,
    UiFormMeta? form,
  }) {
    final result = UiSettingSpec._();
    if (key != null) result.key = key;
    if (dbKey != null) result.dbKey = dbKey;
    if (kind != null) result.kind = kind;
    if (mac != null) result.mac = mac;
    if (win != null) result.win = win;
    if (form != null) result.form = form;
    return result;
  }

  UiSettingSpec._();

  factory UiSettingSpec.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiSettingSpec()..mergeFromBuffer(data, registry);
  factory UiSettingSpec.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiSettingSpec()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiSettingSpec',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiSettingSpec.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'key')
    ..aOS(2, _omitFieldNames ? '' : 'dbKey')
    ..aOS(3, _omitFieldNames ? '' : 'kind')
    ..aOB(4, _omitFieldNames ? '' : 'mac')
    ..aOB(5, _omitFieldNames ? '' : 'win')
    ..aOM<UiFormMeta>(6, _omitFieldNames ? '' : 'form',
        subBuilder: UiFormMeta.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiSettingSpec clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiSettingSpec copyWith(void Function(UiSettingSpec) updates) =>
      super.copyWith((message) => updates(message as UiSettingSpec))
          as UiSettingSpec;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiSettingSpec() / UiSettingSpec.new instead')
  static UiSettingSpec create() => UiSettingSpec._();
  static $pb.GeneratedMessage $_createMessage() => UiSettingSpec._();
  @$core.override
  UiSettingSpec createEmptyInstance() => UiSettingSpec._();
  @$core.pragma('dart2js:noInline')
  static UiSettingSpec getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiSettingSpec>(
          UiSettingSpec.$_createMessage);
  static UiSettingSpec? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get key => $_getSZ(0);
  @$pb.TagNumber(1)
  set key($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearKey() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get dbKey => $_getSZ(1);
  @$pb.TagNumber(2)
  set dbKey($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDbKey() => $_has(1);
  @$pb.TagNumber(2)
  void clearDbKey() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get kind => $_getSZ(2);
  @$pb.TagNumber(3)
  set kind($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasKind() => $_has(2);
  @$pb.TagNumber(3)
  void clearKind() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get mac => $_getBF(3);
  @$pb.TagNumber(4)
  set mac($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasMac() => $_has(3);
  @$pb.TagNumber(4)
  void clearMac() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.bool get win => $_getBF(4);
  @$pb.TagNumber(5)
  set win($core.bool value) => $_setBool(4, value);
  @$pb.TagNumber(5)
  $core.bool hasWin() => $_has(4);
  @$pb.TagNumber(5)
  void clearWin() => $_clearField(5);

  @$pb.TagNumber(6)
  UiFormMeta get form => $_getN(5);
  @$pb.TagNumber(6)
  set form(UiFormMeta value) => $_setField(6, value);
  @$pb.TagNumber(6)
  $core.bool hasForm() => $_has(5);
  @$pb.TagNumber(6)
  void clearForm() => $_clearField(6);
  @$pb.TagNumber(6)
  UiFormMeta ensureForm() => $_ensure(5);
}

class UiFormMeta extends $pb.GeneratedMessage {
  factory UiFormMeta({
    $core.String? key,
    $core.String? section,
    $core.int? order,
    $core.String? labelId,
    $core.String? descriptionId,
    $core.String? hintId,
    $core.String? control,
    $core.String? layout,
    $core.bool? showLabel,
    $core.bool? indented,
    $core.bool? dividerBefore,
    $core.String? unit,
    UiFormRange? range,
    $core.Iterable<UiFormOption>? options,
    $core.String? visibleWhenFeature,
    UiSettingCondition? visibleWhenSetting,
    UiSettingCondition? enabledWhenSetting,
    $core.bool? advanced,
  }) {
    final result = UiFormMeta._();
    if (key != null) result.key = key;
    if (section != null) result.section = section;
    if (order != null) result.order = order;
    if (labelId != null) result.labelId = labelId;
    if (descriptionId != null) result.descriptionId = descriptionId;
    if (hintId != null) result.hintId = hintId;
    if (control != null) result.control = control;
    if (layout != null) result.layout = layout;
    if (showLabel != null) result.showLabel = showLabel;
    if (indented != null) result.indented = indented;
    if (dividerBefore != null) result.dividerBefore = dividerBefore;
    if (unit != null) result.unit = unit;
    if (range != null) result.range = range;
    if (options != null) result.options.addAll(options);
    if (visibleWhenFeature != null)
      result.visibleWhenFeature = visibleWhenFeature;
    if (visibleWhenSetting != null)
      result.visibleWhenSetting = visibleWhenSetting;
    if (enabledWhenSetting != null)
      result.enabledWhenSetting = enabledWhenSetting;
    if (advanced != null) result.advanced = advanced;
    return result;
  }

  UiFormMeta._();

  factory UiFormMeta.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormMeta()..mergeFromBuffer(data, registry);
  factory UiFormMeta.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormMeta()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiFormMeta',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiFormMeta.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'key')
    ..aOS(2, _omitFieldNames ? '' : 'section')
    ..aI(3, _omitFieldNames ? '' : 'order')
    ..aOS(4, _omitFieldNames ? '' : 'labelId')
    ..aOS(5, _omitFieldNames ? '' : 'descriptionId')
    ..aOS(6, _omitFieldNames ? '' : 'hintId')
    ..aOS(7, _omitFieldNames ? '' : 'control')
    ..aOS(8, _omitFieldNames ? '' : 'layout')
    ..aOB(9, _omitFieldNames ? '' : 'showLabel')
    ..aOB(10, _omitFieldNames ? '' : 'indented')
    ..aOB(11, _omitFieldNames ? '' : 'dividerBefore')
    ..aOS(12, _omitFieldNames ? '' : 'unit')
    ..aOM<UiFormRange>(13, _omitFieldNames ? '' : 'range',
        subBuilder: UiFormRange.$_createMessage)
    ..pPM<UiFormOption>(14, _omitFieldNames ? '' : 'options',
        subBuilder: UiFormOption.$_createMessage)
    ..aOS(15, _omitFieldNames ? '' : 'visibleWhenFeature')
    ..aOM<UiSettingCondition>(16, _omitFieldNames ? '' : 'visibleWhenSetting',
        subBuilder: UiSettingCondition.$_createMessage)
    ..aOM<UiSettingCondition>(17, _omitFieldNames ? '' : 'enabledWhenSetting',
        subBuilder: UiSettingCondition.$_createMessage)
    ..aOB(18, _omitFieldNames ? '' : 'advanced')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormMeta clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormMeta copyWith(void Function(UiFormMeta) updates) =>
      super.copyWith((message) => updates(message as UiFormMeta)) as UiFormMeta;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiFormMeta() / UiFormMeta.new instead')
  static UiFormMeta create() => UiFormMeta._();
  static $pb.GeneratedMessage $_createMessage() => UiFormMeta._();
  @$core.override
  UiFormMeta createEmptyInstance() => UiFormMeta._();
  @$core.pragma('dart2js:noInline')
  static UiFormMeta getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UiFormMeta>(UiFormMeta.$_createMessage);
  static UiFormMeta? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get key => $_getSZ(0);
  @$pb.TagNumber(1)
  set key($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearKey() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get section => $_getSZ(1);
  @$pb.TagNumber(2)
  set section($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSection() => $_has(1);
  @$pb.TagNumber(2)
  void clearSection() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get order => $_getIZ(2);
  @$pb.TagNumber(3)
  set order($core.int value) => $_setSignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasOrder() => $_has(2);
  @$pb.TagNumber(3)
  void clearOrder() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get labelId => $_getSZ(3);
  @$pb.TagNumber(4)
  set labelId($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasLabelId() => $_has(3);
  @$pb.TagNumber(4)
  void clearLabelId() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get descriptionId => $_getSZ(4);
  @$pb.TagNumber(5)
  set descriptionId($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasDescriptionId() => $_has(4);
  @$pb.TagNumber(5)
  void clearDescriptionId() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get hintId => $_getSZ(5);
  @$pb.TagNumber(6)
  set hintId($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasHintId() => $_has(5);
  @$pb.TagNumber(6)
  void clearHintId() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get control => $_getSZ(6);
  @$pb.TagNumber(7)
  set control($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasControl() => $_has(6);
  @$pb.TagNumber(7)
  void clearControl() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.String get layout => $_getSZ(7);
  @$pb.TagNumber(8)
  set layout($core.String value) => $_setString(7, value);
  @$pb.TagNumber(8)
  $core.bool hasLayout() => $_has(7);
  @$pb.TagNumber(8)
  void clearLayout() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.bool get showLabel => $_getBF(8);
  @$pb.TagNumber(9)
  set showLabel($core.bool value) => $_setBool(8, value);
  @$pb.TagNumber(9)
  $core.bool hasShowLabel() => $_has(8);
  @$pb.TagNumber(9)
  void clearShowLabel() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.bool get indented => $_getBF(9);
  @$pb.TagNumber(10)
  set indented($core.bool value) => $_setBool(9, value);
  @$pb.TagNumber(10)
  $core.bool hasIndented() => $_has(9);
  @$pb.TagNumber(10)
  void clearIndented() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.bool get dividerBefore => $_getBF(10);
  @$pb.TagNumber(11)
  set dividerBefore($core.bool value) => $_setBool(10, value);
  @$pb.TagNumber(11)
  $core.bool hasDividerBefore() => $_has(10);
  @$pb.TagNumber(11)
  void clearDividerBefore() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.String get unit => $_getSZ(11);
  @$pb.TagNumber(12)
  set unit($core.String value) => $_setString(11, value);
  @$pb.TagNumber(12)
  $core.bool hasUnit() => $_has(11);
  @$pb.TagNumber(12)
  void clearUnit() => $_clearField(12);

  @$pb.TagNumber(13)
  UiFormRange get range => $_getN(12);
  @$pb.TagNumber(13)
  set range(UiFormRange value) => $_setField(13, value);
  @$pb.TagNumber(13)
  $core.bool hasRange() => $_has(12);
  @$pb.TagNumber(13)
  void clearRange() => $_clearField(13);
  @$pb.TagNumber(13)
  UiFormRange ensureRange() => $_ensure(12);

  @$pb.TagNumber(14)
  $pb.PbList<UiFormOption> get options => $_getList(13);

  @$pb.TagNumber(15)
  $core.String get visibleWhenFeature => $_getSZ(14);
  @$pb.TagNumber(15)
  set visibleWhenFeature($core.String value) => $_setString(14, value);
  @$pb.TagNumber(15)
  $core.bool hasVisibleWhenFeature() => $_has(14);
  @$pb.TagNumber(15)
  void clearVisibleWhenFeature() => $_clearField(15);

  @$pb.TagNumber(16)
  UiSettingCondition get visibleWhenSetting => $_getN(15);
  @$pb.TagNumber(16)
  set visibleWhenSetting(UiSettingCondition value) => $_setField(16, value);
  @$pb.TagNumber(16)
  $core.bool hasVisibleWhenSetting() => $_has(15);
  @$pb.TagNumber(16)
  void clearVisibleWhenSetting() => $_clearField(16);
  @$pb.TagNumber(16)
  UiSettingCondition ensureVisibleWhenSetting() => $_ensure(15);

  @$pb.TagNumber(17)
  UiSettingCondition get enabledWhenSetting => $_getN(16);
  @$pb.TagNumber(17)
  set enabledWhenSetting(UiSettingCondition value) => $_setField(17, value);
  @$pb.TagNumber(17)
  $core.bool hasEnabledWhenSetting() => $_has(16);
  @$pb.TagNumber(17)
  void clearEnabledWhenSetting() => $_clearField(17);
  @$pb.TagNumber(17)
  UiSettingCondition ensureEnabledWhenSetting() => $_ensure(16);

  @$pb.TagNumber(18)
  $core.bool get advanced => $_getBF(17);
  @$pb.TagNumber(18)
  set advanced($core.bool value) => $_setBool(17, value);
  @$pb.TagNumber(18)
  $core.bool hasAdvanced() => $_has(17);
  @$pb.TagNumber(18)
  void clearAdvanced() => $_clearField(18);
}

class UiFormRange extends $pb.GeneratedMessage {
  factory UiFormRange({
    $core.double? min,
    $core.double? max,
    $core.double? step,
  }) {
    final result = UiFormRange._();
    if (min != null) result.min = min;
    if (max != null) result.max = max;
    if (step != null) result.step = step;
    return result;
  }

  UiFormRange._();

  factory UiFormRange.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormRange()..mergeFromBuffer(data, registry);
  factory UiFormRange.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormRange()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiFormRange',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiFormRange.$_createMessage)
    ..aD(1, _omitFieldNames ? '' : 'min')
    ..aD(2, _omitFieldNames ? '' : 'max')
    ..aD(3, _omitFieldNames ? '' : 'step')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormRange clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormRange copyWith(void Function(UiFormRange) updates) =>
      super.copyWith((message) => updates(message as UiFormRange))
          as UiFormRange;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiFormRange() / UiFormRange.new instead')
  static UiFormRange create() => UiFormRange._();
  static $pb.GeneratedMessage $_createMessage() => UiFormRange._();
  @$core.override
  UiFormRange createEmptyInstance() => UiFormRange._();
  @$core.pragma('dart2js:noInline')
  static UiFormRange getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiFormRange>(
          UiFormRange.$_createMessage);
  static UiFormRange? _defaultInstance;

  @$pb.TagNumber(1)
  $core.double get min => $_getN(0);
  @$pb.TagNumber(1)
  set min($core.double value) => $_setDouble(0, value);
  @$pb.TagNumber(1)
  $core.bool hasMin() => $_has(0);
  @$pb.TagNumber(1)
  void clearMin() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.double get max => $_getN(1);
  @$pb.TagNumber(2)
  set max($core.double value) => $_setDouble(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMax() => $_has(1);
  @$pb.TagNumber(2)
  void clearMax() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.double get step => $_getN(2);
  @$pb.TagNumber(3)
  set step($core.double value) => $_setDouble(2, value);
  @$pb.TagNumber(3)
  $core.bool hasStep() => $_has(2);
  @$pb.TagNumber(3)
  void clearStep() => $_clearField(3);
}

class UiFormOption extends $pb.GeneratedMessage {
  factory UiFormOption({
    SettingValue? value,
    $core.String? labelId,
  }) {
    final result = UiFormOption._();
    if (value != null) result.value = value;
    if (labelId != null) result.labelId = labelId;
    return result;
  }

  UiFormOption._();

  factory UiFormOption.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormOption()..mergeFromBuffer(data, registry);
  factory UiFormOption.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormOption()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiFormOption',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiFormOption.$_createMessage)
    ..aOM<SettingValue>(1, _omitFieldNames ? '' : 'value',
        subBuilder: SettingValue.$_createMessage)
    ..aOS(2, _omitFieldNames ? '' : 'labelId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormOption clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormOption copyWith(void Function(UiFormOption) updates) =>
      super.copyWith((message) => updates(message as UiFormOption))
          as UiFormOption;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiFormOption() / UiFormOption.new instead')
  static UiFormOption create() => UiFormOption._();
  static $pb.GeneratedMessage $_createMessage() => UiFormOption._();
  @$core.override
  UiFormOption createEmptyInstance() => UiFormOption._();
  @$core.pragma('dart2js:noInline')
  static UiFormOption getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiFormOption>(
          UiFormOption.$_createMessage);
  static UiFormOption? _defaultInstance;

  @$pb.TagNumber(1)
  SettingValue get value => $_getN(0);
  @$pb.TagNumber(1)
  set value(SettingValue value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasValue() => $_has(0);
  @$pb.TagNumber(1)
  void clearValue() => $_clearField(1);
  @$pb.TagNumber(1)
  SettingValue ensureValue() => $_ensure(0);

  @$pb.TagNumber(2)
  $core.String get labelId => $_getSZ(1);
  @$pb.TagNumber(2)
  set labelId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasLabelId() => $_has(1);
  @$pb.TagNumber(2)
  void clearLabelId() => $_clearField(2);
}

class UiSettingCondition extends $pb.GeneratedMessage {
  factory UiSettingCondition({
    $core.String? key,
    SettingValue? equals,
  }) {
    final result = UiSettingCondition._();
    if (key != null) result.key = key;
    if (equals != null) result.equals = equals;
    return result;
  }

  UiSettingCondition._();

  factory UiSettingCondition.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiSettingCondition()..mergeFromBuffer(data, registry);
  factory UiSettingCondition.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiSettingCondition()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiSettingCondition',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiSettingCondition.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'key')
    ..aOM<SettingValue>(2, _omitFieldNames ? '' : 'equals',
        subBuilder: SettingValue.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiSettingCondition clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiSettingCondition copyWith(void Function(UiSettingCondition) updates) =>
      super.copyWith((message) => updates(message as UiSettingCondition))
          as UiSettingCondition;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiSettingCondition() / UiSettingCondition.new instead')
  static UiSettingCondition create() => UiSettingCondition._();
  static $pb.GeneratedMessage $_createMessage() => UiSettingCondition._();
  @$core.override
  UiSettingCondition createEmptyInstance() => UiSettingCondition._();
  @$core.pragma('dart2js:noInline')
  static UiSettingCondition getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UiSettingCondition>(
          UiSettingCondition.$_createMessage);
  static UiSettingCondition? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get key => $_getSZ(0);
  @$pb.TagNumber(1)
  set key($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearKey() => $_clearField(1);

  @$pb.TagNumber(2)
  SettingValue get equals => $_getN(1);
  @$pb.TagNumber(2)
  set equals(SettingValue value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasEquals() => $_has(1);
  @$pb.TagNumber(2)
  void clearEquals() => $_clearField(2);
  @$pb.TagNumber(2)
  SettingValue ensureEquals() => $_ensure(1);
}

class UiFormSection extends $pb.GeneratedMessage {
  factory UiFormSection({
    $core.String? id,
    $core.String? labelId,
    $core.bool? advanced,
    $core.int? order,
    UiSettingCondition? visibleWhenSetting,
  }) {
    final result = UiFormSection._();
    if (id != null) result.id = id;
    if (labelId != null) result.labelId = labelId;
    if (advanced != null) result.advanced = advanced;
    if (order != null) result.order = order;
    if (visibleWhenSetting != null)
      result.visibleWhenSetting = visibleWhenSetting;
    return result;
  }

  UiFormSection._();

  factory UiFormSection.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormSection()..mergeFromBuffer(data, registry);
  factory UiFormSection.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiFormSection()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiFormSection',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiFormSection.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'id')
    ..aOS(2, _omitFieldNames ? '' : 'labelId')
    ..aOB(3, _omitFieldNames ? '' : 'advanced')
    ..aI(4, _omitFieldNames ? '' : 'order')
    ..aOM<UiSettingCondition>(5, _omitFieldNames ? '' : 'visibleWhenSetting',
        subBuilder: UiSettingCondition.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormSection clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiFormSection copyWith(void Function(UiFormSection) updates) =>
      super.copyWith((message) => updates(message as UiFormSection))
          as UiFormSection;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiFormSection() / UiFormSection.new instead')
  static UiFormSection create() => UiFormSection._();
  static $pb.GeneratedMessage $_createMessage() => UiFormSection._();
  @$core.override
  UiFormSection createEmptyInstance() => UiFormSection._();
  @$core.pragma('dart2js:noInline')
  static UiFormSection getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiFormSection>(
          UiFormSection.$_createMessage);
  static UiFormSection? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get id => $_getSZ(0);
  @$pb.TagNumber(1)
  set id($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasId() => $_has(0);
  @$pb.TagNumber(1)
  void clearId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get labelId => $_getSZ(1);
  @$pb.TagNumber(2)
  set labelId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasLabelId() => $_has(1);
  @$pb.TagNumber(2)
  void clearLabelId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get advanced => $_getBF(2);
  @$pb.TagNumber(3)
  set advanced($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasAdvanced() => $_has(2);
  @$pb.TagNumber(3)
  void clearAdvanced() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.int get order => $_getIZ(3);
  @$pb.TagNumber(4)
  set order($core.int value) => $_setSignedInt32(3, value);
  @$pb.TagNumber(4)
  $core.bool hasOrder() => $_has(3);
  @$pb.TagNumber(4)
  void clearOrder() => $_clearField(4);

  @$pb.TagNumber(5)
  UiSettingCondition get visibleWhenSetting => $_getN(4);
  @$pb.TagNumber(5)
  set visibleWhenSetting(UiSettingCondition value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasVisibleWhenSetting() => $_has(4);
  @$pb.TagNumber(5)
  void clearVisibleWhenSetting() => $_clearField(5);
  @$pb.TagNumber(5)
  UiSettingCondition ensureVisibleWhenSetting() => $_ensure(4);
}

class UiCustomBlock extends $pb.GeneratedMessage {
  factory UiCustomBlock({
    $core.String? id,
    $core.String? section,
    $core.int? order,
    UiSettingCondition? visibleWhenSetting,
  }) {
    final result = UiCustomBlock._();
    if (id != null) result.id = id;
    if (section != null) result.section = section;
    if (order != null) result.order = order;
    if (visibleWhenSetting != null)
      result.visibleWhenSetting = visibleWhenSetting;
    return result;
  }

  UiCustomBlock._();

  factory UiCustomBlock.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiCustomBlock()..mergeFromBuffer(data, registry);
  factory UiCustomBlock.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      UiCustomBlock()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UiCustomBlock',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: UiCustomBlock.$_createMessage)
    ..aOS(1, _omitFieldNames ? '' : 'id')
    ..aOS(2, _omitFieldNames ? '' : 'section')
    ..aI(3, _omitFieldNames ? '' : 'order')
    ..aOM<UiSettingCondition>(4, _omitFieldNames ? '' : 'visibleWhenSetting',
        subBuilder: UiSettingCondition.$_createMessage)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiCustomBlock clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UiCustomBlock copyWith(void Function(UiCustomBlock) updates) =>
      super.copyWith((message) => updates(message as UiCustomBlock))
          as UiCustomBlock;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use UiCustomBlock() / UiCustomBlock.new instead')
  static UiCustomBlock create() => UiCustomBlock._();
  static $pb.GeneratedMessage $_createMessage() => UiCustomBlock._();
  @$core.override
  UiCustomBlock createEmptyInstance() => UiCustomBlock._();
  @$core.pragma('dart2js:noInline')
  static UiCustomBlock getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<UiCustomBlock>(
          UiCustomBlock.$_createMessage);
  static UiCustomBlock? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get id => $_getSZ(0);
  @$pb.TagNumber(1)
  set id($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasId() => $_has(0);
  @$pb.TagNumber(1)
  void clearId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get section => $_getSZ(1);
  @$pb.TagNumber(2)
  set section($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSection() => $_has(1);
  @$pb.TagNumber(2)
  void clearSection() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get order => $_getIZ(2);
  @$pb.TagNumber(3)
  set order($core.int value) => $_setSignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasOrder() => $_has(2);
  @$pb.TagNumber(3)
  void clearOrder() => $_clearField(3);

  @$pb.TagNumber(4)
  UiSettingCondition get visibleWhenSetting => $_getN(3);
  @$pb.TagNumber(4)
  set visibleWhenSetting(UiSettingCondition value) => $_setField(4, value);
  @$pb.TagNumber(4)
  $core.bool hasVisibleWhenSetting() => $_has(3);
  @$pb.TagNumber(4)
  void clearVisibleWhenSetting() => $_clearField(4);
  @$pb.TagNumber(4)
  UiSettingCondition ensureVisibleWhenSetting() => $_ensure(3);
}

class SettingsPatch extends $pb.GeneratedMessage {
  factory SettingsPatch({
    $core.Iterable<$core.MapEntry<$core.String, SettingValue>>? values,
  }) {
    final result = SettingsPatch._();
    if (values != null) result.values.addEntries(values);
    return result;
  }

  SettingsPatch._();

  factory SettingsPatch.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsPatch()..mergeFromBuffer(data, registry);
  factory SettingsPatch.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      SettingsPatch()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SettingsPatch',
      package: const $pb.PackageName(_omitMessageNames ? '' : 'glaspen.ipc.v1'),
      createEmptyInstance: SettingsPatch.$_createMessage)
    ..m<$core.String, SettingValue>(1, _omitFieldNames ? '' : 'values',
        entryClassName: 'SettingsPatch.ValuesEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OM,
        valueCreator: SettingValue.$_createMessage,
        valueDefaultOrMaker: SettingValue.getDefault,
        packageName: const $pb.PackageName('glaspen.ipc.v1'))
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsPatch clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SettingsPatch copyWith(void Function(SettingsPatch) updates) =>
      super.copyWith((message) => updates(message as SettingsPatch))
          as SettingsPatch;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  @$core.Deprecated('Use SettingsPatch() / SettingsPatch.new instead')
  static SettingsPatch create() => SettingsPatch._();
  static $pb.GeneratedMessage $_createMessage() => SettingsPatch._();
  @$core.override
  SettingsPatch createEmptyInstance() => SettingsPatch._();
  @$core.pragma('dart2js:noInline')
  static SettingsPatch getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<SettingsPatch>(
          SettingsPatch.$_createMessage);
  static SettingsPatch? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbMap<$core.String, SettingValue> get values => $_getMap(0);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
