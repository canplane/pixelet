// @xpute/spec/lib.rs

pub mod control;
pub mod io;
pub mod ipc;

// Note: `mod lib` in lib.rs would be ambiguous (compiler may resolve to lib.rs itself).
// Use explicit #[path] to point to the lib/ subdirectory.
#[path = "lib/mod.rs"]
pub mod lib;

pub mod rpc;
pub mod sys;
