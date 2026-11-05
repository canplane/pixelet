// pxl-kernel/door/apply/fs.rs

//! FS: the kernel's files: a root mounted, a refused file truncated.

use super::{or_refuse, refuse_unless};
use crate::domain::control::cmd as abi;
use crate::domain::layer::object::repository;
use crate::domain::view::setting::store;
use crate::sys::io::file::{self, StorageFile};
use crate::sys::io::file_spec::root;
use crate::sys::io::source;
use xpute_guest::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn mount(c: abi::FsMount<'_>) -> Outcome {
    refuse_unless(
        source::declare(c.top_zoom, c.format, c.shard.iter()).is_ok(),
        Errno::EINVAL,
    )?;
    Ok(Some(0))
}

pub(super) fn truncated(c: abi::FsTruncated<'_>) -> Outcome {
    match c.root {
        root::STORAGE => {
            let file = or_refuse(
                c.path
                    .parse()
                    .ok()
                    .and_then(StorageFile::of_slot)
                    .filter(|&k| file::is_refused(k)),
                Errno::EINVAL,
            )?;
            file::on_emptied(file);
            // The settings write what they hold from the next keep on.
            match file {
                StorageFile::Settings => store::on_settings_emptied(),
            }
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
