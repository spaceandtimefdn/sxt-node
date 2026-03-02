#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

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
