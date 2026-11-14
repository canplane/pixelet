// xpute-gpu-native-runtime/lib.rs

//! The GPU's bindings on a native device's host's side, which its host
//! (xpute-gpu-native-host) serves on xpute-native-runtime's link, as xpute's
//! own sit in that runtime: generated, and running nothing.

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of the GPU's packages on a native device's host's side,
/// generated from gpu/wit (xpute-gpu-gen-targets).
pub mod abi;
