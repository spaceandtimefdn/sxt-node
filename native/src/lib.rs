//! Space and Time's crate for no_std code that is needed in the runtime and is made available through generated WASM bindings
#![cfg_attr(not(feature = "std"), no_std)]

/// The space and time native code interface
mod sxt;

/// Expose the interface generated from the macro
pub use sxt::interface;
/// These host functions are used at the service level in the node, they allow the connection between our native code and the wasm executor
#[cfg(feature = "std")]
pub use sxt::interface::HostFunctions;
