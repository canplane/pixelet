// xpute-kit/wire/xtp.rs

//! XTP, the Xpute Tree Packet: a tree of values in one buffer, read in place.
//! The format is described in `kit/ts/src/wire/xtp/README.md`, and the golden
//! records under `spec/golden/wire/xtp/` hold the Rust, C++ and TypeScript
//! sides to the same bytes.

pub mod cursor;
pub mod encode;
pub mod shape;
pub mod spec;
pub mod view;

pub use spec::*;

pub use encode::*;
pub use view::*;

pub use cursor::*;
