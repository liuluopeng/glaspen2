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

import 'package:protobuf/protobuf.dart' as $pb;

class CanvasAction extends $pb.ProtobufEnum {
  static const CanvasAction CANVAS_CURRENT =
      CanvasAction._(0, _omitEnumNames ? '' : 'CANVAS_CURRENT');
  static const CanvasAction CANVAS_HOME =
      CanvasAction._(1, _omitEnumNames ? '' : 'CANVAS_HOME');
  static const CanvasAction CANVAS_CENTER =
      CanvasAction._(2, _omitEnumNames ? '' : 'CANVAS_CENTER');
  static const CanvasAction CANVAS_NEW =
      CanvasAction._(3, _omitEnumNames ? '' : 'CANVAS_NEW');

  static const $core.List<CanvasAction> values = <CanvasAction>[
    CANVAS_CURRENT,
    CANVAS_HOME,
    CANVAS_CENTER,
    CANVAS_NEW,
  ];

  static final $core.List<CanvasAction?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 3);
  static CanvasAction? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const CanvasAction._(super.value, super.name);
}

class BackupCommand extends $pb.ProtobufEnum {
  static const BackupCommand BACKUP_NOW =
      BackupCommand._(0, _omitEnumNames ? '' : 'BACKUP_NOW');
  static const BackupCommand RESTORE_LATEST =
      BackupCommand._(1, _omitEnumNames ? '' : 'RESTORE_LATEST');

  static const $core.List<BackupCommand> values = <BackupCommand>[
    BACKUP_NOW,
    RESTORE_LATEST,
  ];

  static final $core.List<BackupCommand?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 1);
  static BackupCommand? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const BackupCommand._(super.value, super.name);
}

const $core.bool _omitEnumNames =
    $core.bool.fromEnvironment('protobuf.omit_enum_names');
