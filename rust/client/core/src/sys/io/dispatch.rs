// pxl-core/sys/io/dispatch.rs

//! What the requests start and stop (sys/io/request.rs `Event`), handed to
//! its reader: an asset's to `loader`, a tile's, a shard's objects or their
//! top to `fetch`. A key is the requests' name for an attempt while it is
//! out.

use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::file::{self, Owner};
use crate::sys::io::{fetch, loader};

pub fn start(key: u64, stream: Stream, id: u64) {
    match stream.row().source {
        Source::Load => {
            if let Some((_, load)) = loader::of_id(id) {
                loader::open_asset(key, load);
            }
        }
        Source::Bake(_) | Source::Objects => fetch::start(key, stream, id),
    }
}

/// Its open closed where it has not answered yet. The requests ended the
/// attempt as they stopped it: what its reader hears of it since is dropped.
pub fn stop(key: u64) {
    let of_key =
        |o: &Owner| matches!(*o, Owner::Fetch { key: k, .. } | Owner::Load { key: k } if k == key);
    if let Some(fd) = file::find(of_key) {
        file::close(fd);
    }
}
