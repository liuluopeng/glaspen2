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
    'CgNhY2sYDSABKAsyEy5nbGFzcGVuLmlwYy52MS5BY2tIAFIDYWNrEi0KBXBhbmVsGBQgASgLMh'
    'UuZ2xhc3Blbi5pcGMudjEuUGFuZWxIAFIFcGFuZWwSLQoFZXJyb3IYYyABKAsyFS5nbGFzcGVu'
    'LmlwYy52MS5FcnJvckgAUgVlcnJvckIGCgRib2R5');

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

@$core.Deprecated('Use settingsSnapshotDescriptor instead')
const SettingsSnapshot$json = {
  '1': 'SettingsSnapshot',
  '2': [
    {
      '1': 'values',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.glaspen.ipc.v1.SettingsSnapshot.ValuesEntry',
      '10': 'values'
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
  '3': [SettingsSnapshot_ValuesEntry$json, SettingsSnapshot_MetadataEntry$json],
};

@$core.Deprecated('Use settingsSnapshotDescriptor instead')
const SettingsSnapshot_ValuesEntry$json = {
  '1': 'ValuesEntry',
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
  '7': {'7': true},
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
    'ChBTZXR0aW5nc1NuYXBzaG90EkQKBnZhbHVlcxgBIAMoCzIsLmdsYXNwZW4uaXBjLnYxLlNldH'
    'RpbmdzU25hcHNob3QuVmFsdWVzRW50cnlSBnZhbHVlcxJKCghtZXRhZGF0YRgCIAMoCzIuLmds'
    'YXNwZW4uaXBjLnYxLlNldHRpbmdzU25hcHNob3QuTWV0YWRhdGFFbnRyeVIIbWV0YWRhdGEaVw'
    'oLVmFsdWVzRW50cnkSEAoDa2V5GAEgASgJUgNrZXkSMgoFdmFsdWUYAiABKAsyHC5nbGFzcGVu'
    'LmlwYy52MS5TZXR0aW5nVmFsdWVSBXZhbHVlOgI4ARo7Cg1NZXRhZGF0YUVudHJ5EhAKA2tleR'
    'gBIAEoCVIDa2V5EhQKBXZhbHVlGAIgASgJUgV2YWx1ZToCOAE=');

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
