// pxl-kernel/abi.rs

//! What the kernel and the shell agree on, defined once in cmd.json: the
//! commands by number, the packet each carries, and the enums and records
//! those are made of. The words are the shell's; what crosses is the number.
//! What any guest and host agree on is `xpute_runtime::abi`'s.

#[path = "abi/cmd.spec.rs"]
pub mod cmd;
