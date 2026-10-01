// pxl-kernel/world.rs

//! The world the kernel keeps: the pages resident in the client, the one
//! query every reader of them asks, the chunks meshed out of them, and the
//! frame's own view of it all. The world's revisions are here too, so a
//! rebuild knows what moved rather than rebuilding on a guess.

pub mod access;
pub mod change;
pub mod chunk;
pub mod frame;
pub mod observation;
pub mod observer;
pub mod paging;
pub mod prefetch;
pub mod pyramid;
pub mod quality;

#[cfg(test)]
#[path = "world/fixture.test.rs"]
pub(crate) mod fixture;
