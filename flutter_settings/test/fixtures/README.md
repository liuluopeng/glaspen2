# uiConfig schema 固定样例(测试用)

这两个 JSON 是 **core 的输出快照**, 供 Dart widget 测试在没有 FRB/管道连接时
拿到"真实 schema"(分区/控件/条件/档位/平台可用性)来验证 schema 渲染器。

- `ui_config_macos.json` / `ui_config_windows.json` = `glaspen_core::ui::ui_config_json`
  对应平台的原样输出(单行 JSON, 字段顺序无关)。
- 漂移闸门: `crates/glaspen-core/src/ui.rs` 的测试
  `flutter_schema_fixtures_match_core_output` 逐字段比对 core 输出与 fixture,
  schema 改了忘记重新生成会直接红。

## 重新生成

```bash
cd <repo root>
cat > crates/glaspen-core/examples/dump_ui_config.rs <<'EOF'
fn main() {
    println!("@@MACOS@@{}", glaspen_core::ui::ui_config_json(glaspen_core::ui::Platform::Macos));
    println!("@@WINDOWS@@{}", glaspen_core::ui::ui_config_json(glaspen_core::ui::Platform::Windows));
}
EOF
cargo run -q -p glaspen-core --example dump_ui_config > /tmp/ui_config_dump.txt
rm -rf crates/glaspen-core/examples
python3 - <<'EOF'
src = open('/tmp/ui_config_dump.txt', encoding='utf-8').read()
for tag, out in [('MACOS','flutter_settings/test/fixtures/ui_config_macos.json'), ('WINDOWS','flutter_settings/test/fixtures/ui_config_windows.json')]:
    marker = '@@'+tag+'@@'
    i = src.index(marker) + len(marker)
    j = src.index('\n', i)
    open(out, 'w', encoding='utf-8').write(src[i:j] + '\n')
EOF
```
