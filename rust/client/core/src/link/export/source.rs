// pxl-core/link/export/source.rs

//! The source interface's method: the tile source the client declares.

use crate::sys::io::source;
use xpute_kit::status::errno::Errno;

pub(super) fn declare(min_zoom: u32, formats: Vec<u32>, shards: Vec<String>) -> Result<(), Errno> {
    source::declare(min_zoom, &formats, shards.iter().map(String::as_str))
}
