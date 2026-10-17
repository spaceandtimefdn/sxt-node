#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", warn(unused_crate_dependencies))]

#[expect(
    mismatched_lifetime_syntaxes,
    non_snake_case,
    non_camel_case_types,
    unused_imports,
    missing_docs,
    clippy::missing_safety_doc,
    clippy::extra_unused_lifetimes,
    clippy::derivable_impls,
    clippy::doc_lazy_continuation
)]
#[rustfmt::skip]
#[path = "../target/flatbuffers/Message_generated.rs"]
pub mod generated;

mod stream_parser_combinators;
pub use stream_parser_combinators::{
    finish,
    single_batch_stream_parser,
    ArrowMessageParseError,
    ArrowRecordBatchParseError,
    ArrowSchemaParseError,
    FinishParseError,
    SingleBatchStream,
    SingleBatchStreamParseError,
};

/// Included here for testing purposes, but in a way that is accessible to downstream that also
/// need similar test/benchmark data.
#[cfg(feature = "std")]
mod write;
#[cfg(feature = "std")]
pub use write::single_batch_stream_bytes;
