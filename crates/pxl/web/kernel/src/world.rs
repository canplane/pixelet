// pxl-kernel/world.rs

//! The world the kernel keeps: the pages resident in the client, the chunks
//! meshed out of them, the height under a point, and the frame's own view of
//! it all. The change log is here too, so a rebuild knows what moved rather
//! than rebuilding on a guess.

pub mod change;
pub mod chunk;
pub mod frame;
pub mod page_table;
pub mod pyramid;
pub mod surface_height;
