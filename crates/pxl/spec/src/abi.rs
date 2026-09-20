// pxl-spec/abi.rs

//! What the kernel and the shell agree on by number, defined once: the
//! commands and the packet each carries, and the messages the kernel raises.
//! The words are the shell's; what crosses is the number. What any guest
//! and host agree on is `xpute_runtime::abi`'s.

#[path = "abi/cmd.spec.rs"]
pub mod cmd;
pub mod message;
