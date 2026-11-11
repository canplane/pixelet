// pxl-core/door/apply/source.rs

//! The SOURCE major's command: the tile source the client declares
//! (SOURCE_DECLARE).

use super::refuse_unless;
use crate::door::cmd as abi;
use crate::sys::io::source;
use xpute_core::io::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn declare(c: abi::SourceDeclare<'_>) -> Outcome {
    refuse_unless(
        source::declare(c.min_zoom, c.format, c.shard.iter()).is_ok(),
        Errno::EINVAL,
    )?;
    Ok(Some(0))
}
