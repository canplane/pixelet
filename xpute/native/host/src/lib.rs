// xpute-native-host/lib.rs

//! The host's end of xpute on a native device, paired file by file with
//! @xpute/web-host: what it serves the guest (`stop`, `clock`). It runs in
//! its runtime (xpute-native-runtime), on the guest's thread, and mounts what
//! it serves on the runtime's link as servers the program lists
//! (xpute-native-runtime native.rs `Boot`), as @xpute/web-host's are on a
//! page.

#![cfg(not(target_arch = "wasm32"))]

pub mod clock;
pub mod stop;
