// pxl-kernel/shell/config/route.rs

//! What the kernel knows of where things are, which is nothing.
//!
//! It named URLs once. It does not now: a read asks for an object by the bake
//! it belongs to and its name inside that bake (`io/fetch.rs`), and the host
//! is the one that knows what address that is and what protocol reaches it.
//! A kernel names what it wants; a driver knows the bus.
//!
//! What is left is the one thing that is not an address — which bake a
//! category belongs to — re-exported so `io/demand.rs` and the shell agree on
//! the number, since both read it from `spec/pxl/spec/bake/published.json`.

pub use pxl_spec::bake::published::{BakeKind as TileKind, PUBLISHED_VERSION};
