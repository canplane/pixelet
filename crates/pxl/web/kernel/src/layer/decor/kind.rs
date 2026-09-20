// pxl-kernel/layer/decor/kind.rs

//! The kinds themselves, one file each, from what kind.json declares: what
//! model a kind uses, what it may stand on, and how thickly it scatters.
//! Adding one is a file here and a line in the spec, and nothing else.

pub mod boat;
pub mod car;
#[path = "kind/kind.spec.rs"]
pub mod spec;
pub mod train;
pub mod tree;
