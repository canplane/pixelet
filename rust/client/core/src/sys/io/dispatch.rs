// pxl-core/sys/io/dispatch.rs

//! Hands each attempt that the request scheduler starts or stops
//! (sys/io/request.rs `Event`) to the reader of its source: an asset to
//! `loader`; a tile, a shard of objects or the objects' top to `fetch`. A key
//! names an attempt only while the attempt is out.

use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::file;
use crate::sys::io::{fetch, loader};
use crate::sys::load;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

pub(crate) fn start(key: u64, stream: Stream, id: u64) {
    match stream.row().source {
        Source::Load => {
            // An asset's id is one `load::id_of` made.
            let (kind, load) = load::of_id(id).or_bug(Errno::ENOTRECOVERABLE);
            loader::open_asset(key, kind, load);
        }
        Source::Base(_) | Source::Object => fetch::start(key, stream, id),
    }
}

/// Closes the attempt's file where its read is out, which cancels it, and
/// lets go of its answer where one waited to be taken, so a body does not
/// outlast its credit. The scheduler ended the attempt when it stopped it,
/// so whatever the reader hears of the attempt afterwards is dropped.
pub(crate) fn stop(key: u64) {
    file::stop(key);
    fetch::drop_stopped();
}
