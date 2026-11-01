// pxl-kernel/door/apply/setting.rs

//! SETTING: a setting's.

use super::State;
use super::{ask_turn, given, refuse_unless};
use crate::domain::control::cmd::{self as abi, Command};
use crate::domain::view::setting::store::are_settings_loaded;
use crate::domain::view::setting::store::settings_store;
use crate::domain::view::setting::store::SettingKey;
use crate::perf::window::perf_follow;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

pub(super) fn apply(
    s: &mut State,
    cmd: Command,
    pkt: &Option<&[u8]>,
) -> Result<Option<i32>, MarshalError> {
    match cmd {
        Command::SETTING_SET => {
            let p = abi::SettingSet::read(given(pkt)?)?;
            // A change before the slot is read would be read over.
            refuse_unless(are_settings_loaded(), Errno::EAGAIN)?;
            let changed = settings_store().set_setting(p.key, p.value);
            refuse_unless(changed.is_some(), Errno::EINVAL)?;
            if changed == Some(true) && p.key == SettingKey::perf {
                perf_follow(p.value == 1);
            }
            ask_turn(s)
        }
        _ => Ok(None),
    }
}
