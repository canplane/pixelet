// pxl-core/door/apply/fs.rs

//! A file changed under the core (xpute's FS_CHANGED): one the core refused
//! and the client has since set aside, which the core reads again.

use super::{or_refuse, refuse_unless};
use crate::domain::layer::object::repository;
use crate::domain::setting::store;
use crate::sys::io::file::{self, File};
use xpute_core::abi::fs::FsChanged;
use xpute_core::io::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(in crate::door) fn changed(c: FsChanged<'_>) -> Outcome {
    match or_refuse(file::of_path(c.path), Errno::EINVAL)? {
        File::Storage => {
            refuse_unless(file::is_storage_refused(), Errno::EINVAL)?;
            file::on_storage_emptied();
            // From the next keep on, the settings write what they hold.
            store::on_settings_emptied();
        }
        // An objects' file is read again, as its source now holds it.
        File::Objects(id) => refuse_unless(repository::on_emptied(id), Errno::EINVAL)?,
    }
    Ok(Some(0))
}
