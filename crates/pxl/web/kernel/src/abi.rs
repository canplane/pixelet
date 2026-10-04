// pxl-kernel/abi.rs

//! What the kernel and the shell agree on, generated from cmd.json,
//! host.json and directory.json. What any guest and host share is
//! `xpute_runtime::abi`'s.

#[path = "abi/cmd.spec.rs"]
pub mod cmd;
#[path = "abi/directory.spec.rs"]
pub mod directory;
#[path = "abi/host.spec.rs"]
pub mod host;
