# glaspen2 日常命令入口(细节见 AGENTS.md「构建与调试命令」)
# just / just --list 查看全部; 参数带 <> 必填, [] 可选

default: list

# 列出全部命令
@list:
    just --list

# 整体构建(build.rs 会编 ObjC → fvm 建 Flutter framework → 链接)
build:
    cargo build

# 运行主程序(Release AOT)
run:
    cargo run

# 调试运行: Flutter 侧走 Debug JIT, 可被 flutter attach
run-debug:
    GLASPEN2_FLUTTER_CONFIG=Debug cargo run

# 本仓测试(根壳 + core + windows)
test:
    cargo test

# 全部可移植仓的测试(排除 installer)
test-all:
    cargo test --workspace --exclude glaspen2_installer

# 聊天协议层测试
test-chat:
    cargo test -p glaspen-chat

# 面板静态检查(0 error/warning 才算过)
analyze:
    cd flutter_settings && fvm flutter analyze

# 面板测试
flutter-test:
    cd flutter_settings && fvm flutter test

# 改了 Rust API 后的 FRB 重新生成(随后必须 build —— content hash 对齐)
codegen:
    cd flutter_settings && flutter_rust_bridge_codegen generate
    cargo build
    @echo "codeGen + build 完成"

# frb_generated.rs 编不过时的自救: 删掉旧生成文件再现算一份全新的
codegen-rescue:
    rm -f src/frb_generated.rs
    cd flutter_settings && flutter_rust_bridge_codegen generate
    cargo build

# 全量自检(提交前): fmt/clippy/测试/analyze/flutter test, 见 scripts/lint.sh
lint:
    sh scripts/lint.sh

# 打包 DMG
dmg:
    sh scripts/build-dmg.sh
