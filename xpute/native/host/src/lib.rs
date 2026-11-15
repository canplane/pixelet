// xpute-native-host/lib.rs

//! The host's end of xpute on a native device, paired file by file with
//! @xpute/web-host: the bindings of xpute's packages on the host's side
//! (`abi`), and what it serves the guest itself (`stop`, `clock`, `log`), as
//! servers the program lists, each mounted on the side its runtime makes and
//! gives it (xpute-link `Side`), the log for its own records too. It names no
//! runtime: what it needs of the runtime it runs in, the runtime gives it
//! (`runtime`), and a job another thread runs comes back to the guest's thread
//! through it (`job`).

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of xpute's packages on a native device's host's side, a
/// module to a package, generated from their WIT (xpute-link-gen).
pub mod abi;
pub mod clock;
pub mod job;
pub mod log;
pub mod runtime;
pub mod stop;
