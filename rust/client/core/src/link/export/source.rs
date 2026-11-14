// pxl-core/link/export/source.rs

//! The source interface's method: the tile source the client declares.

use super::refuse_unless;
use crate::abi::types::Declaration;
use crate::sys::io::source;
use xpute_kit::status::errno::Errno;

pub(super) fn declare(decl: Declaration) -> Result<(), Errno> {
    refuse_unless(source::declare(decl).is_ok(), Errno::EINVAL)?;
    Ok(())
}
