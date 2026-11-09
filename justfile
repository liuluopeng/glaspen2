# glaspen2 日常命令入口(细节见 AGENTS.md「构建与调试命令」)
# just / just --list 查看全部; 参数带 <> 必填, [] 可选
# 注意: just 的规则是注释(doc)在前、[group] 属性在后, 紧贴配方名

default: list

# 列出全部命令
@list:
    just --list

# ── 开发 ──

# 整体构建(build.rs 会编 ObjC → fvm 建 Flutter framework → 链接)
[group('dev')]
build:
    cargo build

# 运行主程序(Release AOT)
[group('dev')]
run:
    cargo run

# 调试运行: Flutter 侧走 Debug JIT, 可被 flutter attach
[group('dev')]
run-debug:
    GLASPEN2_FLUTTER_CONFIG=Debug cargo run

# 改了 Rust API 后的 FRB 重新生成(随后必须 build —— content hash 对齐)
[group('dev')]
codegen:
    cd flutter_settings && flutter_rust_bridge_codegen generate
    cargo build
    @echo "codeGen + build 完成"

# frb_generated.rs 编不过时的自救: 删掉旧生成文件再现算一份全新的
[group('dev')]
codegen-rescue:
    rm -f src/frb_generated.rs
    cd flutter_settings && flutter_rust_bridge_codegen generate
    cargo build

# ── 测试与质量 ──

# 本仓测试(根壳 + core + windows)
[group('quality')]
test:
    cargo test

# 全部可移植仓的测试(排除 installer)
[group('quality')]
test-all:
    cargo test --workspace --exclude glaspen2_installer

# 聊天协议层测试
[group('quality')]
test-chat:
    cargo test -p glaspen-chat

# 面板静态检查(0 error/warning 才算过)
[group('quality')]
analyze:
    cd flutter_settings && fvm flutter analyze

# 面板测试
[group('quality')]
flutter-test:
    cd flutter_settings && fvm flutter test

# 全量自检(提交前): fmt/clippy/测试/analyze/flutter test, 见 scripts/lint.sh
[group('quality')]
lint:
    sh scripts/lint.sh

# ── 发布 ──

# 打包 DMG
[group('release')]
dmg:
    sh scripts/build-dmg.sh
