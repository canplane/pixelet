// xpute-host/lib.rs

//! The native host's end of xpute, paired file by file with @xpute/host: the
//! bindings of xpute's packages on the host's side (`abi`), and what it
//! implements for the guest itself (`stop`, `clock`, `log`), as mounts the
//! program lists, each mounted on the side its runtime makes and gives it
//! (xpute-link `Side`), the log for its own records too. It names no runtime:
//! what it needs of the runtime it runs in, the runtime gives it (`runtime`).

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of xpute's packages on the native host's side, a module to a
/// package, generated from their WIT (xpute-link-gen).
pub mod abi;
pub mod clock;
pub mod log;
pub mod runtime;
pub mod stop;
