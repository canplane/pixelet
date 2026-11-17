// xpute-link/lib.rs

//! xpute's link, the protocol both its sides run, and its code: a frame's
//! words (`frame`), the link machine each side runs one of (link.rs, its
//! items here at the root), the handle table (`resource`), a sent method as a
//! future (`wait`), the memory a method lends (`mem`), a packet's values in
//! the Canonical ABI's layout (cabi.rs, its `Lower` and `Lift` here at the
//! root), and the link's own types (`abi`).

pub mod abi;
mod cabi;
pub mod frame;
mod link;
pub mod mem;
pub mod resource;
pub mod wait;

pub use cabi::{Lift, Lower};
pub use link::{
    Answer, Args, CANCEL, Cancel, DROP, Handle, Layout, Lend, Method, NAMED, OPEN, OPEN_AT, Object,
    Packet, ROOT, Return, Side, TRAP, TRAP_AT, Tag, answer, layout, load_uri, store_uri, uri,
};
