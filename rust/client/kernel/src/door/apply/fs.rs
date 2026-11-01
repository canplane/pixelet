// pxl-kernel/door/apply/fs.rs

//! FS: the kernel's files: a root mounted, a storage file truncated.

use super::State;
use super::{given, or_refuse, refuse_unless};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::layer::object::repository;
use crate::domain::view::setting::store;
use crate::sys::io::source;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    _s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::FS_MOUNT => {
            let p = abi::FsMount::read(given(pkt)?)?;
            refuse_unless(
                source::declare(p.top_zoom, p.format, p.shard.iter()).is_ok(),
                Errno::EINVAL,
            )?;
            Ok(Some(0))
        }
        Command::FS_TRUNCATED => {
            use crate::sys::io::file::{self, StorageFile};
            let p = abi::FsTruncated::read(given(pkt)?)?;
            let file = or_refuse(
                StorageFile::of_slot(p.slot).filter(|&k| file::is_refused(k)),
                Errno::EINVAL,
            )?;
            file::on_emptied(file);
            // The settings write what they hold from the next keep on; the
            // objects are read again, empty, and saved from then on.
            match file {
                StorageFile::Settings => store::on_settings_emptied(),
                StorageFile::Objects => repository::open(),
            }
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}
