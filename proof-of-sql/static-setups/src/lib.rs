#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "baked")]
pub mod baked;
