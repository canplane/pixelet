// pxl-core/door/apply/fs.rs

//! A file changed under the core (xpute's FS_CHANGED): one the core refused
//! and the client has since set aside, which the core reads again.

use super::{or_refuse, refuse_unless};
use crate::domain::layer::object::repository;
use crate::domain::setting::store;
use crate::sys::io::file;
use crate::sys::io::file_spec::{root, storage};
use xpute_core::abi::fs::FsChanged;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(in crate::door) fn changed(c: FsChanged<'_>) -> Outcome {
    match c.root {
        root::STORAGE => {
            refuse_unless(
                c.path.parse::<u32>() == Ok(storage::SETTINGS) && file::is_storage_refused(),
                Errno::EINVAL,
            )?;
            file::on_storage_emptied();
            // From the next keep on, the settings write what they hold.
            store::on_settings_emptied();
        }
        // The object file is read again, as its source now holds it.
        root::OBJECT => {
            let id = or_refuse(file::id_of_path(c.path), Errno::EINVAL)?;
            refuse_unless(repository::on_emptied(id), Errno::EINVAL)?;
        }
        _ => refuse_unless(false, Errno::EINVAL)?,
    }
    Ok(Some(0))
}
