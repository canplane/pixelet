// xpute-link/lib.rs

//! xpute's link, the protocol both its sides run, and its code: a frame's
//! words (`frame`), the link machine each side runs one of (link.rs, its
//! items here at the root), the handle table (`resource`), a sent method as a
//! future (`wait`), the memory a method lends (`mem`), and the link's own
//! types (`abi`).

pub mod abi;
pub mod frame;
mod link;
pub mod mem;
pub mod resource;
pub mod wait;

pub use link::{
    answer, uri, Answer, Args, Data, Handle, Layout, Lend, Method, Object, Packet, Reply, Return,
    Side, Tag, CANCEL, DROP, NAMED, OPEN, ROOT, TRAP,
};
