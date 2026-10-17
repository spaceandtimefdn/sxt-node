//! Compiles `proto/prover-db.proto` into Rust types via `prost_build`.

fn main() {
    prost_build::Config::new()
        .compile_protos(&["proto/prover-db.proto"], &["proto"])
        .expect("failed to compile proto/prover-db.proto");
    println!("cargo:rerun-if-changed=proto/prover-db.proto");
}
