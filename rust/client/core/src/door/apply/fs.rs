// pxl-core/door/apply/fs.rs

//! The core's files: the tile source declared (SOURCE_DECLARE), and a file
//! changed under it (xpute's FS_CHANGED): a refused one set aside, read again.

use super::{or_refuse, refuse_unless};
use crate::domain::layer::object::repository;
use crate::domain::view::setting::store;
use crate::door::cmd as abi;
use crate::sys::io::file;
use crate::sys::io::file_spec::{root, storage};
use crate::sys::io::source;
use xpute_core::abi::fs::FsChanged;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn declare(c: abi::SourceDeclare<'_>) -> Outcome {
    refuse_unless(
        source::declare(c.top_zoom, c.format, c.shard.iter()).is_ok(),
        Errno::EINVAL,
    )?;
    Ok(Some(0))
}

/// A file changed under the core (FS_CHANGED).
pub(in crate::door) fn changed(c: FsChanged<'_>) -> Outcome {
    match c.root {
        root::STORAGE => {
            refuse_unless(
                c.path.parse::<u32>() == Ok(storage::SETTINGS) && file::is_storage_refused(),
                Errno::EINVAL,
            )?;
            file::on_storage_emptied();
            // The settings write what they hold from the next keep on.
            store::on_settings_emptied();
        }
        // Read again, as its source holds it now.
        root::OBJECT => {
            let id = or_refuse(file::id_of_path(c.path), Errno::EINVAL)?;
            refuse_unless(repository::on_emptied(id), Errno::EINVAL)?;
        }
        _ => refuse_unless(false, Errno::EINVAL)?,
    }
    Ok(Some(0))
}
