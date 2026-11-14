// pxl-core/link/export/setting.rs

//! The setting interface's methods.

use super::{applied, or_refuse, refuse_unless};
use crate::abi::types::ErrorCode;
use crate::domain::setting::are_settings_loaded;
use crate::domain::setting::set_dev;
use crate::domain::setting::settings_mut;
use crate::domain::setting::SettingKey;
use crate::perf::window::perf_follow;
use xpute_kit::status::errno::Errno;

pub(super) fn set(key: u32, val: u32) -> Result<Result<(), ErrorCode>, Errno> {
    // A change made before the settings' slot is read would be overwritten
    // by the read, so it is refused until then.
    if !are_settings_loaded() {
        return Ok(Err(ErrorCode::WouldBlock));
    }
    let key = or_refuse(SettingKey::of(key), Errno::EINVAL)?;
    let changed = settings_mut().set_setting(key, val);
    refuse_unless(changed.is_some(), Errno::EINVAL)?;
    if changed == Some(true) && key == SettingKey::Perf {
        perf_follow(val == 1);
    }
    applied().map(Ok)
}

/// Whether this is a development build. The UI sends this once the core is
/// attached.
pub(super) fn dev(dev: bool) -> Result<(), Errno> {
    set_dev(dev);
    Ok(())
}
