fn main() {
    tonic_build::configure()
        // server 代码一并提供,便于本地 axum 服务直接实现 ChatStore。
        .build_server(true)
        .compile_protos(&["../proto/glaspen/chat/v1/chat.proto"], &["../proto"])
        .expect("failed to compile glaspen.chat.v1 proto");
}
