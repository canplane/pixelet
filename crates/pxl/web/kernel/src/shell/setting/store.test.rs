// pxl-kernel/shell/setting/store.test.rs

use std::sync::Mutex;

use super::*;
use crate::global::serial;
use crate::host::{set_development, set_storage, Storage};

/// The one value storage holds under the settings' key, in place of a
/// browser's.
static STORED: Mutex<Option<String>> = Mutex::new(None);

fn under(development: bool, stored: Option<&str>) {
    set_development(development);
    *STORED.lock().unwrap() = stored.map(str::to_string);
    set_storage(Storage {
        get: |_| STORED.lock().unwrap().clone(),
        set: |_, value| *STORED.lock().unwrap() = Some(value.to_string()),
    });
}

fn stored() -> Option<String> {
    STORED.lock().unwrap().clone()
}

/// What differs from the defaults is stored by name and read back; a value
/// set back to its default leaves storage.
#[test]
fn what_is_stored_is_what_differs_and_it_is_read_back() {
    let _g = serial();
    under(false, None);
    let mut s = Settings::DEFAULT;
    s.set_setting(SettingKey::ao, AoMode::screen as u32);
    s.set_setting(SettingKey::person_variant, 3);
    assert_eq!(stored().as_deref(), Some(r#"{"ao":"screen","person_variant":3}"#));
    let back = load();
    assert_eq!(back.ao, AoMode::screen);
    assert_eq!(back.person_variant, 3);
    s.set_setting(SettingKey::ao, Settings::DEFAULT.ao as u32);
    assert_eq!(stored().as_deref(), Some(r#"{"person_variant":3}"#));
}
