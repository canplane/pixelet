// pxl-core/link/export/source.rs

//! The SOURCE major's command: the tile source the client declares
//! (SOURCE_DECLARE).

use super::refuse_unless;
use crate::sys::io::source;
use xpute_kit::status::errno::Errno;

pub(super) fn declare(min_zoom: u32, format: Vec<u32>, shard: Vec<String>) -> Result<(), Errno> {
    refuse_unless(
        source::declare(min_zoom, &format, shard.iter().map(String::as_str)).is_ok(),
        Errno::EINVAL,
    )?;
    Ok(())
}
