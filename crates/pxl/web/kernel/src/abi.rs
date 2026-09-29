// pxl-kernel/abi.rs

//! What the kernel and the shell agree on, defined once in cmd.json: the
//! commands by number, the packet each carries, and the enums and records
//! those are made of; in host.json, the records of the host stream; and in
//! directory.json, the kernel's own entries in its directory. The
//! words are the shell's; what crosses is the number.
//! What any guest and host agree on is `xpute_runtime::abi`'s.

#[path = "abi/cmd.spec.rs"]
pub mod cmd;
#[path = "abi/directory.spec.rs"]
pub mod directory;
#[path = "abi/host.spec.rs"]
pub mod host;
