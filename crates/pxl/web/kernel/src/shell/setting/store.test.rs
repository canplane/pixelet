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

/// The sheet is not the only way a setting is set: the command reaches the
/// store from the console too, so the build's offer has to be checked here
/// and not only on what storage brought in.
#[test]
fn a_setting_the_build_does_not_offer_is_not_set_live() {
    let _g = serial();
    under(false, None);
    let mut s = Settings::DEFAULT;
    s.set_setting(SettingKey::show_debug_objects, 1);
    s.toggle_setting(SettingKey::enable_terrain_carving);
    assert!(!s.show_debug_objects);
    assert!(!s.enable_terrain_carving);
    assert_eq!(stored(), None);
    // A setting every build offers still goes through.
    s.toggle_setting(SettingKey::lock_camera_tilt);
    assert!(s.lock_camera_tilt);
    assert_eq!(stored().as_deref(), Some(r#"{"lock_camera_tilt":true}"#));
}

#[test]
fn a_value_a_setting_cannot_hold_is_refused() {
    let _g = serial();
    under(false, None);
    let mut s = Settings::DEFAULT;
    s.set_setting(SettingKey::lock_camera_tilt, 2);
    s.set_setting(SettingKey::ao, AoMode::ALL.len() as u32);
    s.set_setting(SettingKey::person_variant, 256);
    assert_eq!(s, Settings::DEFAULT);
    assert_eq!(stored(), None);
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

/// A stored value the build cannot reach is left out, and what is left is
/// written back, so storage holds only what the page can undo.
#[test]
fn a_load_leaves_out_what_the_build_cannot_reach_and_writes_the_rest_back() {
    let _g = serial();
    under(false, Some(r#"{"show_debug_objects":true,"ao":"sideways","gone":1,"lock_camera_tilt":true}"#));
    let s = load();
    assert!(!s.show_debug_objects);
    assert_eq!(s.ao, Settings::DEFAULT.ao);
    assert!(s.lock_camera_tilt);
    assert_eq!(stored().as_deref(), Some(r#"{"lock_camera_tilt":true}"#));
}

#[cfg(feature = "dev")]
#[test]
fn a_development_build_offers_its_dev_settings() {
    let _g = serial();
    under(true, None);
    let mut s = Settings::DEFAULT;
    s.toggle_setting(SettingKey::show_poi);
    assert!(s.show_poi);
    assert_eq!(stored().as_deref(), Some(r#"{"show_poi":true}"#));
}

/// Built without the feature, a dev setting is not named in the module at
/// all — not merely not offered — and runs at its default.
#[cfg(not(feature = "dev"))]
#[test]
fn a_module_without_the_feature_names_no_dev_setting() {
    let _g = serial();
    assert!(SettingKey::ALL.into_iter().filter(|k| k.dev()).all(|k| k.name().is_none()));
    under(true, None);
    let mut s = Settings::DEFAULT;
    s.toggle_setting(SettingKey::show_poi);
    assert!(!s.show_poi);
}
