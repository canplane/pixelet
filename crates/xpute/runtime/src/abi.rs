// xpute-runtime/abi.rs

//! What the guest and its host agree on by number: every program's commands,
//! generated from `spec/xpute/abi/` into each language; and a handle to what the host keeps
//! — the table that issues one here, the slot read off it there, and the split
//! between them a number both sides write.

#[path = "abi/cmd.spec.rs"]
pub mod cmd;
pub mod handle;
