// This is a generated file - do not edit.
//
// Generated from glaspen/ipc/v1/ipc.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports
// ignore_for_file: unused_import

import 'dart:convert' as $convert;
import 'dart:core' as $core;
import 'dart:typed_data' as $typed_data;

@$core.Deprecated('Use frameDescriptor instead')
const Frame$json = {
  '1': 'Frame',
  '2': [
    {'1': 'protocol_version', '3': 1, '4': 1, '5': 13, '10': 'protocolVersion'},
    {'1': 'request_id', '3': 2, '4': 1, '5': 4, '10': 'requestId'},
    {
      '1': 'settings_snapshot',
      '3': 10,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshot',
      '9': 0,
      '10': 'settingsSnapshot'
    },
    {
      '1': 'set_setting',
      '3': 11,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SetSetting',
      '9': 0,
      '10': 'setSetting'
    },
    {
      '1': 'settings_snapshot_response',
      '3': 12,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshotResponse',
      '9': 0,
      '10': 'settingsSnapshotResponse'
    },
    {
      '1': 'ack',
      '3': 13,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.Ack',
      '9': 0,
      '10': 'ack'
    },
    {
      '1': 'settings_changed',
      '3': 14,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsChanged',
      '9': 0,
      '10': 'settingsChanged'
    },
    {
      '1': 'page_list_response',
      '3': 15,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.PageListResponse',
      '9': 0,
      '10': 'pageListResponse'
    },
    {
      '1': 'page_thumbnails_response',
      '3': 16,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.PageThumbnailsResponse',
      '9': 0,
      '10': 'pageThumbnailsResponse'
    },
    {
      '1': 'bubble_messages',
      '3': 17,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.BubbleMessages',
      '9': 0,
      '10': 'bubbleMessages'
    },
    {
      '1': 'panel',
      '3': 20,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.Panel',
      '9': 0,
      '10': 'panel'
    },
    {
      '1': 'error',
      '3': 99,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.Error',
      '9': 0,
      '10': 'error'
    },
  ],
  '8': [
    {'1': 'body'},
  ],
};

/// Descriptor for `Frame`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List frameDescriptor = $convert.base64Decode(
    'CgVGcmFtZRIpChBwcm90b2NvbF92ZXJzaW9uGAEgASgNUg9wcm90b2NvbFZlcnNpb24SHQoKcm'
    'VxdWVzdF9pZBgCIAEoBFIJcmVxdWVzdElkEk8KEXNldHRpbmdzX3NuYXBzaG90GAogASgLMiAu'
    'Z2xhc3Blbi5pcGMudjEuU2V0dGluZ3NTbmFwc2hvdEgAUhBzZXR0aW5nc1NuYXBzaG90Ej0KC3'
    'NldF9zZXR0aW5nGAsgASgLMhouZ2xhc3Blbi5pcGMudjEuU2V0U2V0dGluZ0gAUgpzZXRTZXR0'
    'aW5nEmgKGnNldHRpbmdzX3NuYXBzaG90X3Jlc3BvbnNlGAwgASgLMiguZ2xhc3Blbi5pcGMudj'
    'EuU2V0dGluZ3NTbmFwc2hvdFJlc3BvbnNlSABSGHNldHRpbmdzU25hcHNob3RSZXNwb25zZRIn'
    'CgNhY2sYDSABKAsyEy5nbGFzcGVuLmlwYy52MS5BY2tIAFIDYWNrEkwKEHNldHRpbmdzX2NoYW'
    '5nZWQYDiABKAsyHy5nbGFzcGVuLmlwYy52MS5TZXR0aW5nc0NoYW5nZWRIAFIPc2V0dGluZ3ND'
    'aGFuZ2VkElAKEnBhZ2VfbGlzdF9yZXNwb25zZRgPIAEoCzIgLmdsYXNwZW4uaXBjLnYxLlBhZ2'
    'VMaXN0UmVzcG9uc2VIAFIQcGFnZUxpc3RSZXNwb25zZRJiChhwYWdlX3RodW1ibmFpbHNfcmVz'
    'cG9uc2UYECABKAsyJi5nbGFzcGVuLmlwYy52MS5QYWdlVGh1bWJuYWlsc1Jlc3BvbnNlSABSFn'
    'BhZ2VUaHVtYm5haWxzUmVzcG9uc2USSQoPYnViYmxlX21lc3NhZ2VzGBEgASgLMh4uZ2xhc3Bl'
    'bi5pcGMudjEuQnViYmxlTWVzc2FnZXNIAFIOYnViYmxlTWVzc2FnZXMSLQoFcGFuZWwYFCABKA'
    'syFS5nbGFzcGVuLmlwYy52MS5QYW5lbEgAUgVwYW5lbBItCgVlcnJvchhjIAEoCzIVLmdsYXNw'
    'ZW4uaXBjLnYxLkVycm9ySABSBWVycm9yQgYKBGJvZHk=');

@$core.Deprecated('Use settingValueDescriptor instead')
const SettingValue$json = {
  '1': 'SettingValue',
  '2': [
    {'1': 'bool_value', '3': 1, '4': 1, '5': 8, '9': 0, '10': 'boolValue'},
    {'1': 'int_value', '3': 2, '4': 1, '5': 3, '9': 0, '10': 'intValue'},
    {'1': 'double_value', '3': 3, '4': 1, '5': 1, '9': 0, '10': 'doubleValue'},
    {'1': 'string_value', '3': 4, '4': 1, '5': 9, '9': 0, '10': 'stringValue'},
    {
      '1': 'struct_value',
      '3': 5,
      '4': 1,
      '5': 11,
      '6': '.google.protobuf.Struct',
      '9': 0,
      '10': 'structValue'
    },
    {
      '1': 'list_value',
      '3': 6,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.ListValue',
      '9': 0,
      '10': 'listValue'
    },
    {'1': 'bytes_value', '3': 7, '4': 1, '5': 12, '9': 0, '10': 'bytesValue'},
    {'1': 'null_value', '3': 8, '4': 1, '5': 8, '9': 0, '10': 'nullValue'},
  ],
  '8': [
    {'1': 'value'},
  ],
};

/// Descriptor for `SettingValue`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List settingValueDescriptor = $convert.base64Decode(
    'CgxTZXR0aW5nVmFsdWUSHwoKYm9vbF92YWx1ZRgBIAEoCEgAUglib29sVmFsdWUSHQoJaW50X3'
    'ZhbHVlGAIgASgDSABSCGludFZhbHVlEiMKDGRvdWJsZV92YWx1ZRgDIAEoAUgAUgtkb3VibGVW'
    'YWx1ZRIjCgxzdHJpbmdfdmFsdWUYBCABKAlIAFILc3RyaW5nVmFsdWUSPAoMc3RydWN0X3ZhbH'
    'VlGAUgASgLMhcuZ29vZ2xlLnByb3RvYnVmLlN0cnVjdEgAUgtzdHJ1Y3RWYWx1ZRI6CgpsaXN0'
    'X3ZhbHVlGAYgASgLMhkuZ2xhc3Blbi5pcGMudjEuTGlzdFZhbHVlSABSCWxpc3RWYWx1ZRIhCg'
    'tieXRlc192YWx1ZRgHIAEoDEgAUgpieXRlc1ZhbHVlEh8KCm51bGxfdmFsdWUYCCABKAhIAFIJ'
    'bnVsbFZhbHVlQgcKBXZhbHVl');

@$core.Deprecated('Use listValueDescriptor instead')
const ListValue$json = {
  '1': 'ListValue',
  '2': [
    {
      '1': 'values',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingValue',
      '10': 'values'
    },
  ],
};

/// Descriptor for `ListValue`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listValueDescriptor = $convert.base64Decode(
    'CglMaXN0VmFsdWUSNAoGdmFsdWVzGAEgAygLMhwuZ2xhc3Blbi5pcGMudjEuU2V0dGluZ1ZhbH'
    'VlUgZ2YWx1ZXM=');

@$core.Deprecated('Use appSettingsDescriptor instead')
const AppSettings$json = {
  '1': 'AppSettings',
  '2': [
    {'1': 'color', '3': 1, '4': 1, '5': 5, '10': 'color'},
    {'1': 'width', '3': 2, '4': 1, '5': 5, '10': 'width'},
    {'1': 'launch_at_login', '3': 3, '4': 1, '5': 8, '10': 'launchAtLogin'},
    {'1': 'frosted_glass', '3': 4, '4': 1, '5': 8, '10': 'frostedGlass'},
    {'1': 'grid', '3': 5, '4': 1, '5': 8, '10': 'grid'},
    {
      '1': 'grid_follow_strokes',
      '3': 6,
      '4': 1,
      '5': 8,
      '10': 'gridFollowStrokes'
    },
    {
      '1': 'glass_follow_strokes',
      '3': 7,
      '4': 1,
      '5': 8,
      '10': 'glassFollowStrokes'
    },
    {'1': 'soft_shadow', '3': 8, '4': 1, '5': 8, '10': 'softShadow'},
    {'1': 'invert_ink', '3': 9, '4': 1, '5': 8, '10': 'invertInk'},
    {'1': 'notebook_style', '3': 10, '4': 1, '5': 8, '10': 'notebookStyle'},
    {
      '1': 'panel_transparent',
      '3': 11,
      '4': 1,
      '5': 8,
      '10': 'panelTransparent'
    },
    {'1': 'invert_fps', '3': 12, '4': 1, '5': 5, '10': 'invertFps'},
    {'1': 'pressure_monitor', '3': 13, '4': 1, '5': 8, '10': 'pressureMonitor'},
    {'1': 'outline', '3': 14, '4': 1, '5': 8, '10': 'outline'},
    {'1': 'infinite_canvas', '3': 15, '4': 1, '5': 8, '10': 'infiniteCanvas'},
    {'1': 'minimap', '3': 16, '4': 1, '5': 8, '10': 'minimap'},
    {'1': 'grid_size', '3': 17, '4': 1, '5': 1, '10': 'gridSize'},
    {'1': 'grid_divider', '3': 18, '4': 1, '5': 5, '10': 'gridDivider'},
    {'1': 'flip_effect', '3': 19, '4': 1, '5': 5, '10': 'flipEffect'},
    {'1': 'gif_fps', '3': 20, '4': 1, '5': 5, '10': 'gifFps'},
    {'1': 'gif_resolution', '3': 21, '4': 1, '5': 1, '10': 'gifResolution'},
    {'1': 'gif_speed', '3': 22, '4': 1, '5': 1, '10': 'gifSpeed'},
    {'1': 'gif_end_mode', '3': 23, '4': 1, '5': 5, '10': 'gifEndMode'},
    {'1': 'chat_api_base', '3': 24, '4': 1, '5': 9, '10': 'chatApiBase'},
    {'1': 'chat_user', '3': 25, '4': 1, '5': 9, '10': 'chatUser'},
    {
      '1': 'chat_has_password',
      '3': 26,
      '4': 1,
      '5': 8,
      '10': 'chatHasPassword'
    },
    {'1': 'chat_integration', '3': 27, '4': 1, '5': 8, '10': 'chatIntegration'},
    {'1': 'show_free_canvas', '3': 28, '4': 1, '5': 8, '10': 'showFreeCanvas'},
    {'1': 'show_bubble_tab', '3': 29, '4': 1, '5': 8, '10': 'showBubbleTab'},
    {
      '1': 'bubble_replay_speed',
      '3': 30,
      '4': 1,
      '5': 5,
      '10': 'bubbleReplaySpeed'
    },
    {
      '1': 'bubble_replay_hold_last',
      '3': 31,
      '4': 1,
      '5': 8,
      '10': 'bubbleReplayHoldLast'
    },
    {
      '1': 'bubble_replay_auto',
      '3': 32,
      '4': 1,
      '5': 8,
      '10': 'bubbleReplayAuto'
    },
    {
      '1': 'bubble_auto_new_page',
      '3': 33,
      '4': 1,
      '5': 8,
      '10': 'bubbleAutoNewPage'
    },
    {'1': 'nas_url', '3': 34, '4': 1, '5': 9, '10': 'nasUrl'},
    {'1': 'share_canvas', '3': 35, '4': 1, '5': 8, '10': 'shareCanvas'},
    {'1': 'language', '3': 36, '4': 1, '5': 9, '10': 'language'},
    {'1': 'shortcuts', '3': 37, '4': 1, '5': 9, '10': 'shortcuts'},
  ],
};

/// Descriptor for `AppSettings`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List appSettingsDescriptor = $convert.base64Decode(
    'CgtBcHBTZXR0aW5ncxIUCgVjb2xvchgBIAEoBVIFY29sb3ISFAoFd2lkdGgYAiABKAVSBXdpZH'
    'RoEiYKD2xhdW5jaF9hdF9sb2dpbhgDIAEoCFINbGF1bmNoQXRMb2dpbhIjCg1mcm9zdGVkX2ds'
    'YXNzGAQgASgIUgxmcm9zdGVkR2xhc3MSEgoEZ3JpZBgFIAEoCFIEZ3JpZBIuChNncmlkX2ZvbG'
    'xvd19zdHJva2VzGAYgASgIUhFncmlkRm9sbG93U3Ryb2tlcxIwChRnbGFzc19mb2xsb3dfc3Ry'
    'b2tlcxgHIAEoCFISZ2xhc3NGb2xsb3dTdHJva2VzEh8KC3NvZnRfc2hhZG93GAggASgIUgpzb2'
    'Z0U2hhZG93Eh0KCmludmVydF9pbmsYCSABKAhSCWludmVydEluaxIlCg5ub3RlYm9va19zdHls'
    'ZRgKIAEoCFINbm90ZWJvb2tTdHlsZRIrChFwYW5lbF90cmFuc3BhcmVudBgLIAEoCFIQcGFuZW'
    'xUcmFuc3BhcmVudBIdCgppbnZlcnRfZnBzGAwgASgFUglpbnZlcnRGcHMSKQoQcHJlc3N1cmVf'
    'bW9uaXRvchgNIAEoCFIPcHJlc3N1cmVNb25pdG9yEhgKB291dGxpbmUYDiABKAhSB291dGxpbm'
    'USJwoPaW5maW5pdGVfY2FudmFzGA8gASgIUg5pbmZpbml0ZUNhbnZhcxIYCgdtaW5pbWFwGBAg'
    'ASgIUgdtaW5pbWFwEhsKCWdyaWRfc2l6ZRgRIAEoAVIIZ3JpZFNpemUSIQoMZ3JpZF9kaXZpZG'
    'VyGBIgASgFUgtncmlkRGl2aWRlchIfCgtmbGlwX2VmZmVjdBgTIAEoBVIKZmxpcEVmZmVjdBIX'
    'CgdnaWZfZnBzGBQgASgFUgZnaWZGcHMSJQoOZ2lmX3Jlc29sdXRpb24YFSABKAFSDWdpZlJlc2'
    '9sdXRpb24SGwoJZ2lmX3NwZWVkGBYgASgBUghnaWZTcGVlZBIgCgxnaWZfZW5kX21vZGUYFyAB'
    'KAVSCmdpZkVuZE1vZGUSIgoNY2hhdF9hcGlfYmFzZRgYIAEoCVILY2hhdEFwaUJhc2USGwoJY2'
    'hhdF91c2VyGBkgASgJUghjaGF0VXNlchIqChFjaGF0X2hhc19wYXNzd29yZBgaIAEoCFIPY2hh'
    'dEhhc1Bhc3N3b3JkEikKEGNoYXRfaW50ZWdyYXRpb24YGyABKAhSD2NoYXRJbnRlZ3JhdGlvbh'
    'IoChBzaG93X2ZyZWVfY2FudmFzGBwgASgIUg5zaG93RnJlZUNhbnZhcxImCg9zaG93X2J1YmJs'
    'ZV90YWIYHSABKAhSDXNob3dCdWJibGVUYWISLgoTYnViYmxlX3JlcGxheV9zcGVlZBgeIAEoBV'
    'IRYnViYmxlUmVwbGF5U3BlZWQSNQoXYnViYmxlX3JlcGxheV9ob2xkX2xhc3QYHyABKAhSFGJ1'
    'YmJsZVJlcGxheUhvbGRMYXN0EiwKEmJ1YmJsZV9yZXBsYXlfYXV0bxggIAEoCFIQYnViYmxlUm'
    'VwbGF5QXV0bxIvChRidWJibGVfYXV0b19uZXdfcGFnZRghIAEoCFIRYnViYmxlQXV0b05ld1Bh'
    'Z2USFwoHbmFzX3VybBgiIAEoCVIGbmFzVXJsEiEKDHNoYXJlX2NhbnZhcxgjIAEoCFILc2hhcm'
    'VDYW52YXMSGgoIbGFuZ3VhZ2UYJCABKAlSCGxhbmd1YWdlEhwKCXNob3J0Y3V0cxglIAEoCVIJ'
    'c2hvcnRjdXRz');

@$core.Deprecated('Use settingsSnapshotDescriptor instead')
const SettingsSnapshot$json = {
  '1': 'SettingsSnapshot',
  '2': [
    {
      '1': 'settings',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.AppSettings',
      '10': 'settings'
    },
    {
      '1': 'metadata',
      '3': 2,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshot.MetadataEntry',
      '10': 'metadata'
    },
  ],
  '3': [SettingsSnapshot_MetadataEntry$json],
};

@$core.Deprecated('Use settingsSnapshotDescriptor instead')
const SettingsSnapshot_MetadataEntry$json = {
  '1': 'MetadataEntry',
  '2': [
    {'1': 'key', '3': 1, '4': 1, '5': 9, '10': 'key'},
    {'1': 'value', '3': 2, '4': 1, '5': 9, '10': 'value'},
  ],
  '7': {'7': true},
};

/// Descriptor for `SettingsSnapshot`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List settingsSnapshotDescriptor = $convert.base64Decode(
    'ChBTZXR0aW5nc1NuYXBzaG90EjcKCHNldHRpbmdzGAEgASgLMhsuZ2xhc3Blbi5pcGMudjEuQX'
    'BwU2V0dGluZ3NSCHNldHRpbmdzEkoKCG1ldGFkYXRhGAIgAygLMi4uZ2xhc3Blbi5pcGMudjEu'
    'U2V0dGluZ3NTbmFwc2hvdC5NZXRhZGF0YUVudHJ5UghtZXRhZGF0YRo7Cg1NZXRhZGF0YUVudH'
    'J5EhAKA2tleRgBIAEoCVIDa2V5EhQKBXZhbHVlGAIgASgJUgV2YWx1ZToCOAE=');

@$core.Deprecated('Use settingsSnapshotResponseDescriptor instead')
const SettingsSnapshotResponse$json = {
  '1': 'SettingsSnapshotResponse',
  '2': [
    {
      '1': 'snapshot',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshot',
      '10': 'snapshot'
    },
  ],
};

/// Descriptor for `SettingsSnapshotResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List settingsSnapshotResponseDescriptor =
    $convert.base64Decode(
        'ChhTZXR0aW5nc1NuYXBzaG90UmVzcG9uc2USPAoIc25hcHNob3QYASABKAsyIC5nbGFzcGVuLm'
        'lwYy52MS5TZXR0aW5nc1NuYXBzaG90UghzbmFwc2hvdA==');

@$core.Deprecated('Use setSettingDescriptor instead')
const SetSetting$json = {
  '1': 'SetSetting',
  '2': [
    {'1': 'key', '3': 1, '4': 1, '5': 9, '10': 'key'},
    {
      '1': 'value',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingValue',
      '10': 'value'
    },
  ],
};

/// Descriptor for `SetSetting`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List setSettingDescriptor = $convert.base64Decode(
    'CgpTZXRTZXR0aW5nEhAKA2tleRgBIAEoCVIDa2V5EjIKBXZhbHVlGAIgASgLMhwuZ2xhc3Blbi'
    '5pcGMudjEuU2V0dGluZ1ZhbHVlUgV2YWx1ZQ==');

@$core.Deprecated('Use ackDescriptor instead')
const Ack$json = {
  '1': 'Ack',
  '2': [
    {'1': 'ok', '3': 1, '4': 1, '5': 8, '10': 'ok'},
  ],
};

/// Descriptor for `Ack`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List ackDescriptor =
    $convert.base64Decode('CgNBY2sSDgoCb2sYASABKAhSAm9r');

@$core.Deprecated('Use errorDescriptor instead')
const Error$json = {
  '1': 'Error',
  '2': [
    {'1': 'code', '3': 1, '4': 1, '5': 9, '10': 'code'},
    {'1': 'message', '3': 2, '4': 1, '5': 9, '10': 'message'},
  ],
};

/// Descriptor for `Error`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List errorDescriptor = $convert.base64Decode(
    'CgVFcnJvchISCgRjb2RlGAEgASgJUgRjb2RlEhgKB21lc3NhZ2UYAiABKAlSB21lc3NhZ2U=');

@$core.Deprecated('Use panelDescriptor instead')
const Panel$json = {
  '1': 'Panel',
  '2': [
    {'1': 'kind', '3': 1, '4': 1, '5': 9, '10': 'kind'},
    {
      '1': 'data',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.google.protobuf.Value',
      '10': 'data'
    },
    {'1': 'ok', '3': 3, '4': 1, '5': 8, '10': 'ok'},
  ],
};

/// Descriptor for `Panel`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List panelDescriptor = $convert.base64Decode(
    'CgVQYW5lbBISCgRraW5kGAEgASgJUgRraW5kEioKBGRhdGEYAiABKAsyFi5nb29nbGUucHJvdG'
    '9idWYuVmFsdWVSBGRhdGESDgoCb2sYAyABKAhSAm9r');

@$core.Deprecated('Use stringMapDescriptor instead')
const StringMap$json = {
  '1': 'StringMap',
  '2': [
    {
      '1': 'values',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.StringMap.ValuesEntry',
      '10': 'values'
    },
  ],
  '3': [StringMap_ValuesEntry$json],
};

@$core.Deprecated('Use stringMapDescriptor instead')
const StringMap_ValuesEntry$json = {
  '1': 'ValuesEntry',
  '2': [
    {'1': 'key', '3': 1, '4': 1, '5': 9, '10': 'key'},
    {'1': 'value', '3': 2, '4': 1, '5': 9, '10': 'value'},
  ],
  '7': {'7': true},
};

/// Descriptor for `StringMap`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List stringMapDescriptor = $convert.base64Decode(
    'CglTdHJpbmdNYXASPQoGdmFsdWVzGAEgAygLMiUuZ2xhc3Blbi5pcGMudjEuU3RyaW5nTWFwLl'
    'ZhbHVlc0VudHJ5UgZ2YWx1ZXMaOQoLVmFsdWVzRW50cnkSEAoDa2V5GAEgASgJUgNrZXkSFAoF'
    'dmFsdWUYAiABKAlSBXZhbHVlOgI4AQ==');

@$core.Deprecated('Use stringListDescriptor instead')
const StringList$json = {
  '1': 'StringList',
  '2': [
    {'1': 'values', '3': 1, '4': 3, '5': 9, '10': 'values'},
  ],
};

/// Descriptor for `StringList`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List stringListDescriptor =
    $convert.base64Decode('CgpTdHJpbmdMaXN0EhYKBnZhbHVlcxgBIAMoCVIGdmFsdWVz');

@$core.Deprecated('Use settingsChangedDescriptor instead')
const SettingsChanged$json = {
  '1': 'SettingsChanged',
  '2': [
    {
      '1': 'snapshot',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshot',
      '10': 'snapshot'
    },
  ],
};

/// Descriptor for `SettingsChanged`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List settingsChangedDescriptor = $convert.base64Decode(
    'Cg9TZXR0aW5nc0NoYW5nZWQSPAoIc25hcHNob3QYASABKAsyIC5nbGFzcGVuLmlwYy52MS5TZX'
    'R0aW5nc1NuYXBzaG90UghzbmFwc2hvdA==');

@$core.Deprecated('Use pageSummaryDescriptor instead')
const PageSummary$json = {
  '1': 'PageSummary',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 3, '10': 'id'},
    {'1': 'width', '3': 2, '4': 1, '5': 5, '10': 'width'},
    {'1': 'height', '3': 3, '4': 1, '5': 5, '10': 'height'},
    {'1': 'stroke_count', '3': 4, '4': 1, '5': 5, '10': 'strokeCount'},
  ],
};

/// Descriptor for `PageSummary`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pageSummaryDescriptor = $convert.base64Decode(
    'CgtQYWdlU3VtbWFyeRIOCgJpZBgBIAEoA1ICaWQSFAoFd2lkdGgYAiABKAVSBXdpZHRoEhYKBm'
    'hlaWdodBgDIAEoBVIGaGVpZ2h0EiEKDHN0cm9rZV9jb3VudBgEIAEoBVILc3Ryb2tlQ291bnQ=');

@$core.Deprecated('Use pageListResponseDescriptor instead')
const PageListResponse$json = {
  '1': 'PageListResponse',
  '2': [
    {
      '1': 'pages',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.PageSummary',
      '10': 'pages'
    },
  ],
};

/// Descriptor for `PageListResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pageListResponseDescriptor = $convert.base64Decode(
    'ChBQYWdlTGlzdFJlc3BvbnNlEjEKBXBhZ2VzGAEgAygLMhsuZ2xhc3Blbi5pcGMudjEuUGFnZV'
    'N1bW1hcnlSBXBhZ2Vz');

@$core.Deprecated('Use pageThumbnailDescriptor instead')
const PageThumbnail$json = {
  '1': 'PageThumbnail',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 3, '10': 'id'},
    {'1': 'png', '3': 2, '4': 1, '5': 12, '10': 'png'},
  ],
};

/// Descriptor for `PageThumbnail`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pageThumbnailDescriptor = $convert.base64Decode(
    'Cg1QYWdlVGh1bWJuYWlsEg4KAmlkGAEgASgDUgJpZBIQCgNwbmcYAiABKAxSA3BuZw==');

@$core.Deprecated('Use pageThumbnailsResponseDescriptor instead')
const PageThumbnailsResponse$json = {
  '1': 'PageThumbnailsResponse',
  '2': [
    {
      '1': 'thumbnails',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.PageThumbnail',
      '10': 'thumbnails'
    },
  ],
};

/// Descriptor for `PageThumbnailsResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pageThumbnailsResponseDescriptor =
    $convert.base64Decode(
        'ChZQYWdlVGh1bWJuYWlsc1Jlc3BvbnNlEj0KCnRodW1ibmFpbHMYASADKAsyHS5nbGFzcGVuLm'
        'lwYy52MS5QYWdlVGh1bWJuYWlsUgp0aHVtYm5haWxz');

@$core.Deprecated('Use bubblePointDescriptor instead')
const BubblePoint$json = {
  '1': 'BubblePoint',
  '2': [
    {'1': 'x', '3': 1, '4': 1, '5': 1, '10': 'x'},
    {'1': 'y', '3': 2, '4': 1, '5': 1, '10': 'y'},
    {'1': 'width', '3': 3, '4': 1, '5': 1, '10': 'width'},
  ],
};

/// Descriptor for `BubblePoint`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List bubblePointDescriptor = $convert.base64Decode(
    'CgtCdWJibGVQb2ludBIMCgF4GAEgASgBUgF4EgwKAXkYAiABKAFSAXkSFAoFd2lkdGgYAyABKA'
    'FSBXdpZHRo');

@$core.Deprecated('Use bubbleStrokeDescriptor instead')
const BubbleStroke$json = {
  '1': 'BubbleStroke',
  '2': [
    {'1': 'rgb', '3': 1, '4': 1, '5': 13, '10': 'rgb'},
    {'1': 'width_scale', '3': 2, '4': 1, '5': 1, '10': 'widthScale'},
    {'1': 'gap_ms', '3': 3, '4': 1, '5': 4, '10': 'gapMs'},
    {
      '1': 'points',
      '3': 4,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.BubblePoint',
      '10': 'points'
    },
  ],
};

/// Descriptor for `BubbleStroke`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List bubbleStrokeDescriptor = $convert.base64Decode(
    'CgxCdWJibGVTdHJva2USEAoDcmdiGAEgASgNUgNyZ2ISHwoLd2lkdGhfc2NhbGUYAiABKAFSCn'
    'dpZHRoU2NhbGUSFQoGZ2FwX21zGAMgASgEUgVnYXBNcxIzCgZwb2ludHMYBCADKAsyGy5nbGFz'
    'cGVuLmlwYy52MS5CdWJibGVQb2ludFIGcG9pbnRz');

@$core.Deprecated('Use bubbleMessageDescriptor instead')
const BubbleMessage$json = {
  '1': 'BubbleMessage',
  '2': [
    {'1': 'id', '3': 1, '4': 1, '5': 4, '10': 'id'},
    {'1': 'ts_ms', '3': 2, '4': 1, '5': 4, '10': 'tsMs'},
    {'1': 'device', '3': 3, '4': 1, '5': 9, '10': 'device'},
    {
      '1': 'strokes',
      '3': 4,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.BubbleStroke',
      '10': 'strokes'
    },
  ],
};

/// Descriptor for `BubbleMessage`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List bubbleMessageDescriptor = $convert.base64Decode(
    'Cg1CdWJibGVNZXNzYWdlEg4KAmlkGAEgASgEUgJpZBITCgV0c19tcxgCIAEoBFIEdHNNcxIWCg'
    'ZkZXZpY2UYAyABKAlSBmRldmljZRI2CgdzdHJva2VzGAQgAygLMhwuZ2xhc3Blbi5pcGMudjEu'
    'QnViYmxlU3Ryb2tlUgdzdHJva2Vz');

@$core.Deprecated('Use bubbleMessagesDescriptor instead')
const BubbleMessages$json = {
  '1': 'BubbleMessages',
  '2': [
    {'1': 'device', '3': 1, '4': 1, '5': 9, '10': 'device'},
    {
      '1': 'messages',
      '3': 2,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.BubbleMessage',
      '10': 'messages'
    },
  ],
};

/// Descriptor for `BubbleMessages`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List bubbleMessagesDescriptor = $convert.base64Decode(
    'Cg5CdWJibGVNZXNzYWdlcxIWCgZkZXZpY2UYASABKAlSBmRldmljZRI5CghtZXNzYWdlcxgCIA'
    'MoCzIdLmdsYXNwZW4uaXBjLnYxLkJ1YmJsZU1lc3NhZ2VSCG1lc3NhZ2Vz');
