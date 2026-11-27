import 'package:flutter_test/flutter_test.dart';
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
