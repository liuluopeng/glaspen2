import 'package:flutter/material.dart';

bool x = true, y = true;
void f(dynamic v) {}
Widget _tile(Widget c) => c;
Widget _buildSection(String t, Widget c) => c;

Widget mini() {
  return Padding(
    padding: const EdgeInsets.fromLTRB(12, 6, 12, 0),
    child: _buildSection('真穿透(实验)', _tile(SwitchListTile(
      value: x && y,
      onChanged: x
          ? (v) {
              f(v);
            }
          : null,
    ))),
  );
}
