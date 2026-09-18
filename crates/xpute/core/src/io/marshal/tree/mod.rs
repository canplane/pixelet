// xpute-core/io/marshal/tree/mod.rs

//! XTP: a relocatable binary tree packet. The wire format and the API it
//! is read and written through are
//! `packages/xpute/core/src/io/marshal/tree/README.md`, which both
//! implementations are held to.

pub mod cursor;
pub mod encode;
pub mod spec;
pub mod view;

pub use spec::*;

// write end
pub use encode::*;
pub use view::*;

// read end
pub use cursor::*;
