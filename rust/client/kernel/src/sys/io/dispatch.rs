// pxl-kernel/sys/io/dispatch.rs

//! What the request scheduler started and stopped (request.rs `Event`),
//! handed to the reader each is for: an asset's to `loader`, every other's, a
//! tile's, a shard's objects or their top, to `fetch`.

use crate::sys::io::demand::Stream;
use crate::sys::io::request::Credit;
use crate::sys::io::{fetch, loader};

/// The key is an opaque handle to the shell.
pub fn start(credit: Credit, stream: Stream, id: u64) {
    match stream {
        Stream::asset => {
            if let Some((_, load)) = loader::of_id(id) {
                loader::open_asset(credit, load);
            }
        }
        _ => fetch::start(credit, stream, id),
    }
}

pub fn stop(key: u64) {
    if !fetch::cancel(key) {
        loader::cancel_asset(key);
    }
}
