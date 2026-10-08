// xpute-kit/wire/xtp.rs

//! XTP, the Xpute Tree Packet: a relocatable binary tree packet. Its format is stated in
//! `ts/kit/src/wire/xtp/README.md`.

pub mod cursor;
pub mod encode;
pub mod spec;
pub mod view;

pub use spec::*;

pub use encode::*;
pub use view::*;

pub use cursor::*;
