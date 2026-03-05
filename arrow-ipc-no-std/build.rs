//! Compiles arrow flatbuffer schemas into a rust library.
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=flatbuffers/*.fbs");
    flatc_rust::run(flatc_rust::Args {
        inputs: &[Path::new("flatbuffers/Message.fbs")],
        out_dir: Path::new("target/flatbuffers/"),
        extra: &["--gen-all"],
        ..Default::default()
    })
    .expect("flatc");
}
