fn main() {
    // 不写这行, proto 变了而 crate 源码没变时 cargo 不会重跑本脚本,
    // 生成的绑定就会停留在旧协议上(CanvasShare 就是这么踩的)。
    println!("cargo:rerun-if-changed=../proto/glaspen/chat/v1/chat.proto");
    tonic_build::configure()
        // server 代码一并提供,便于本地 axum 服务直接实现 ChatStore。
        .build_server(true)
        .compile_protos(&["../proto/glaspen/chat/v1/chat.proto"], &["../proto"])
        .expect("failed to compile glaspen.chat.v1 proto");
}
