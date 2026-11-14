// pxl-core/sys/io/dispatch.rs

//! Hands each attempt that the request scheduler starts or stops
//! (sys/io/request.rs `Event`) to the reader of its source: an asset to
//! `loader`; a tile, a shard of objects or the objects' top to `fetch`. A key
//! names an attempt only while the attempt is out.

use crate::sys::io::demand::{Source, Stream};
use crate::sys::io::file;
use crate::sys::io::{fetch, loader};
use crate::sys::load;

pub fn start(key: u64, stream: Stream, id: u64) {
    match stream.row().source {
        Source::Load => {
            if let Some((_, load)) = load::of_id(id) {
                loader::open_asset(key, load);
            }
        }
        Source::Base(_) | Source::Object => fetch::start(key, stream, id),
    }
}

/// Closes the attempt's file where its read is out, which cancels it. The
/// scheduler ended the attempt when it stopped it, so whatever the reader
/// hears of the attempt afterwards is dropped.
pub fn stop(key: u64) {
    file::stop(key);
}
