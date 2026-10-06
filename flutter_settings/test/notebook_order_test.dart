// 页分组排序纯函数测试: 页重排(order_index)可见性的防线。
// 此前 _groupSorted 组内按 id 二次排序, 把 DB 重排结果整个吞掉
// (前后移按钮"按了没反应"的根因)。
import 'package:flutter_test/flutter_test.dart';
import 'package:glaspen2_settings/main.dart';

PageInfo p(int id, int w, int h) => PageInfo(id: id, w: w, h: h);

void main() {
  test('组内保持传入顺序(order_index), 不按 id 重排', () {
    // DB 顺序: 15 → 11 → 13(用户手动重排过)
    final pages = [p(15, 3440, 1440), p(11, 3440, 1440), p(13, 3440, 1440)];
    final out = groupSortedPages(pages);
    expect(out.map((e) => e.id).toList(), [15, 11, 13],
        reason: '重排结果必须原样透传');
  });

  test('跨组分组, 组间最近活跃在前', () {
    final pages = [
      p(11, 3440, 1440),
      p(12, 3440, 1440),
      p(5, 1920, 1080),
      p(6, 1920, 1080),
    ];
    final out = groupSortedPages(pages);
    // 3440 组(最大 id 12)在前, 1920 组在后; 组内保持传入序
    expect(out.map((e) => e.id).toList(), [11, 12, 5, 6]);
  });

  test('1920 组更活跃时排前', () {
    final pages = [
      p(11, 3440, 1440),
      p(20, 1920, 1080),
    ];
    final out = groupSortedPages(pages);
    expect(out.map((e) => e.id).toList(), [20, 11]);
  });

  test('空列表原样返回', () {
    expect(groupSortedPages(const []), isEmpty);
  });
  test('拖拽提交锚点: 中位 → 移到后邻之前', () {
    final ordered = [p(1, 1, 1), p(2, 1, 1), p(3, 1, 1)];
    final a = pageDragCommitAnchor(ordered, 1); // 拖动 1, 落点在中间(idx 0 有后邻)
    expect(a, (2, true));
  });

  test('拖拽提交锚点: 末位 → 移到前邻之后', () {
    final ordered = [p(1, 1, 1), p(2, 1, 1), p(3, 1, 1)];
    expect(pageDragCommitAnchor(ordered, 3), (2, false));
  });

  test('拖拽提交锚点: 单元素/未知页 → null', () {
    expect(pageDragCommitAnchor([p(1, 1, 1)], 1), isNull);
    expect(pageDragCommitAnchor([p(1, 1, 1), p(2, 1, 1)], 99), isNull);
  });
}
