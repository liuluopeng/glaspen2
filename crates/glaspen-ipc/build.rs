fn main() {
    let proto = "../../proto/glaspen/ipc/v1/ipc.proto";
    let include = "../../proto";
    println!("cargo:rerun-if-changed={proto}");
    prost_build::Config::new()
        .compile_protos(&[proto], &[include])
        .expect("compile glaspen ipc proto");
}
