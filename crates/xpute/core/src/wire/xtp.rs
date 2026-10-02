// xpute-core/wire/xtp.rs

//! XTP, a relocatable binary tree packet. Its format is stated in
//! `packages/xpute/core/src/wire/xtp/README.md`.

pub mod cursor;
pub mod encode;
pub mod spec;
pub mod view;

pub use spec::*;

pub use encode::*;
pub use view::*;

pub use cursor::*;
