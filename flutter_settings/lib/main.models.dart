part of 'main.dart';

// ── 数据模型与画笔 ──

class _ThumbSkeleton extends StatelessWidget {
  const _ThumbSkeleton();

  @override
  Widget build(BuildContext context) => const ColoredBox(color: Color(0x1AF3EEE3));
}

class PageInfo {
  final int id;
  final int w;
  final int h;
  final int strokeCount;
  Uint8List? thumbnail;

  PageInfo({
    required this.id,
    required this.w,
    required this.h,
    this.strokeCount = 0,
  });

  factory PageInfo.fromJson(Map<String, dynamic> json) {
    return PageInfo(
      id: json['id'] as int,
      w: json['w'] as int,
      h: json['h'] as int,
      strokeCount: (json['strokes'] as num?)?.toInt() ?? 0,
    );
  }
}

// ── 纸墨主题 ──
const _paperBg = Color(0xFFF3EEE3);   // 暖纸底(最底层,不透明)
const _paperCard = Color(0x9EFBF8F1); // 区块卡纸面(半透明:让底层插画透出来)
const _ink = Color(0xFF2C2A26);       // 墨色文字
const _inkFaint = Color(0xFF8A857A);  // 淡墨(次要)
const _penRed = Color(0xFFC4353F);    // 笔锋红(强调色)
const _paperLine = Color(0xFFE0D9C8); // 分隔/描边

ThemeData _buildPaperTheme() {
  final scheme = ColorScheme.light(
    primary: _penRed,
    onPrimary: Colors.white,
    secondary: const Color(0xFF0070BD),
    surface: _paperCard,
    onSurface: _ink,
    surfaceContainerHighest: _paperBg,
    outline: const Color(0xFFD8D2C4),
    outlineVariant: const Color(0xFFE4DECF),
  );
  return ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: Colors.transparent,
    fontFamily: 'LXGWWenKaiMono',
    textTheme: ThemeData(brightness: Brightness.light)
        .textTheme
        .apply(
            bodyColor: _ink,
            displayColor: _ink,
            fontFamily: 'LXGWWenKaiMono'),
    appBarTheme: const AppBarTheme(
      backgroundColor: _paperBg,
      foregroundColor: _ink,
      elevation: 0,
      centerTitle: true,
      titleTextStyle: TextStyle(
          fontFamily: 'LXGWWenKaiMono',
          fontSize: 18,
          fontWeight: FontWeight.bold,
          color: _ink),
    ),
    tabBarTheme: const TabBarThemeData(
      labelColor: _ink,
      unselectedLabelColor: _inkFaint,
      indicatorColor: _penRed,
      dividerColor: _paperLine,
    ),
    dividerTheme: const DividerThemeData(color: _paperLine),
    switchTheme: SwitchThemeData(
      thumbColor: WidgetStateProperty.resolveWith(
          (s) => s.contains(WidgetState.selected) ? Colors.white : const Color(0xFFB9B2A2)),
      trackColor: WidgetStateProperty.resolveWith(
          (s) => s.contains(WidgetState.selected) ? _penRed : const Color(0xFFD8D2C4)),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: _ink,
        side: const BorderSide(color: Color(0xFFD8D2C4)),
        backgroundColor: _paperCard,
      ),
    ),
    // 卡片(页面缩略图等)也半透明,与模块卡纸一致
    cardTheme: CardThemeData(
      color: _paperCard,
      elevation: 0,
      shape: RoundedRectangleBorder(
        side: const BorderSide(color: _paperLine),
        borderRadius: BorderRadius.circular(10),
      ),
    ),
  );
}

/// 方格手账纸的点阵背景
class _PaperDotsPainter extends CustomPainter {
  const _PaperDotsPainter();
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = const Color(0x1A2C2A26);
    const gap = 24.0;
    for (double y = gap; y < size.height; y += gap) {
      for (double x = gap; x < size.width; x += gap) {
        canvas.drawCircle(Offset(x, y), 1.0, paint);
      }
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

// ── App ──


/// 页分组排序(纯函数, 可单测):
/// - 按分辨率 key(WxH)分组;
/// - 组内**保持传入顺序**(list_screens 已按 order_index 排 —— 页重排
///   的可见性全靠这里不再二次排序, 此前按 id 重排曾把重排结果整个吞掉);
/// - 组间按最近活跃(组内最大 id)排前。
List<PageInfo> groupSortedPages(List<PageInfo> pages) {
  if (pages.isEmpty) return pages;
  final groups = <String, List<PageInfo>>{};
  for (final p in pages) {
    groups.putIfAbsent('${p.w}x${p.h}', () => []).add(p);
  }
  final keys = groups.keys.toList()
    ..sort((a, b) {
      int mx(String k) => groups[k]!.map((p) => p.id).reduce((x, y) => x > y ? x : y);
      return mx(b).compareTo(mx(a));
    });
  final out = <PageInfo>[];
  for (final k in keys) {
    out.addAll(groups[k]!);
  }
  return out;
}


/// 活页本拖拽提交锚点(纯函数): 有后邻 → "移到后邻之前";
/// 已是末位 → "移到前邻之后"。返回 null = 无法提交(页不存在/只有一个元素)。
(int, bool)? pageDragCommitAnchor(List<PageInfo> ordered, int draggedId) {
  final idx = ordered.indexWhere((p) => p.id == draggedId);
  if (idx < 0 || ordered.length < 2) return null;
  if (idx + 1 < ordered.length) return (ordered[idx + 1].id, true);
  return (ordered[idx - 1].id, false);
}
