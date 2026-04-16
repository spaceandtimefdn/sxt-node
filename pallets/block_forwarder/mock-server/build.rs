fn main() {
    prost_build::Config::new()
        .compile_protos(&["../proto/indexer.proto"], &["../proto"])
        .expect("failed to compile indexer.proto");
    println!("cargo:rerun-if-changed=../proto/indexer.proto");
}
