// pxl-core/door/apply/setting.rs

//! The SETTING major's commands.

use super::State;
use super::{ask_turn, refuse_unless};
use crate::domain::setting::store::are_settings_loaded;
use crate::domain::setting::store::set_dev;
use crate::domain::setting::store::settings_mut;
use crate::domain::setting::store::SettingKey;
use crate::door::cmd as abi;
use crate::perf::window::perf_follow;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn set(s: &mut State, c: abi::SettingSet) -> Outcome {
    // A change made before the settings' slot is read would be overwritten
    // by the read, so it is refused until then.
    refuse_unless(are_settings_loaded(), Errno::EAGAIN)?;
    let changed = settings_mut().set_setting(c.key, c.val);
    refuse_unless(changed.is_some(), Errno::EINVAL)?;
    if changed == Some(true) && c.key == SettingKey::perf {
        perf_follow(c.val == 1);
    }
    ask_turn(s)
}

/// Whether this is a development build. The UI sends this once the core is
/// attached.
pub(super) fn dev(c: abi::SettingDev) -> Outcome {
    set_dev(c.dev);
    Ok(Some(0))
}
