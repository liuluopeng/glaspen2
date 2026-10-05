import 'package:flutter/material.dart';

bool x = true, y = true;
bool _notebookGlass = true, _panelTransparent = false;
void f(dynamic v) {}
void setState(void Function() fn) {}
void _setSetting(String k, dynamic v) {}
Widget _tile(Widget c) => c;
Widget _buildSection(String t, Widget c) => c;

Widget mini2() {
  return Padding(
    padding: const EdgeInsets.fromLTRB(12, 6, 12, 0),
    child: _buildSection('真穿透(实验)', _tile(SwitchListTile(
      title: const Text('卡片透出面板后的桌面', style: TextStyle(fontSize: 14)),
      subtitle: const Text('清玻璃形态: 需拟物玻璃; 关闭恢复模糊+底图', style: TextStyle(fontSize: 12)),
      value: _panelTransparent && _notebookGlass,
      dense: true,
      contentPadding: EdgeInsets.zero,
      onChanged: _notebookGlass
          ? (v) {
              setState(() => _panelTransparent = v);
              _setSetting('panelTransparent', v);
            }
          : null,
    ))),
  );
}
