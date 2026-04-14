//! Compiles `proto/indexer.proto` into Rust types via `prost_build`.
//! Only active with the `std` feature (the offchain worker path).

fn main() {
    #[cfg(feature = "std")]
    {
        prost_build::Config::new()
            .compile_protos(&["proto/indexer.proto"], &["proto"])
            .expect("failed to compile proto/indexer.proto");
    }
    println!("cargo:rerun-if-changed=proto/indexer.proto");
}
