// pxl-kernel/shell/setting/store.test.rs

use super::*;
use crate::global::serial;
use crate::host::set_handheld;
use crate::shell::config::env::{set_build_info, BuildInfo};
use crate::shell::setting::schema::DEFAULT_SETTINGS;

fn store_under(app_env: &str) -> SettingsStore {
    set_build_info(BuildInfo {
        app_env: app_env.to_string(),
        ..BuildInfo::default()
    });
    SettingsStore::defaults()
}

/// The sheet is not the only way a setting is set: the action reaches the
/// store from the console too, so the build's offer has to be checked here
/// and not only on what storage brought in.
#[test]
fn a_setting_the_build_does_not_offer_is_not_set_live() {
    let _g = serial();
    let mut s = store_under("production");
    s.set_setting("show_debug_objects", SettingValue::Toggle(true));
    s.toggle_setting("enable_terrain_carving");
    assert!(!s.show_debug_objects);
    assert!(!s.enable_terrain_carving);
    assert!(s.overrides.is_empty());
    // A setting every build offers still goes through.
    s.toggle_setting("lock_camera_tilt");
    assert!(s.lock_camera_tilt);
    assert_eq!(s.overrides, vec![("lock_camera_tilt", SettingValue::Toggle(true))]);
}

#[cfg(feature = "dev")]
#[test]
fn a_development_build_offers_its_dev_settings() {
    let _g = serial();
    let mut s = store_under("development");
    s.toggle_setting("show_poi");
    assert!(s.show_poi);
    assert_eq!(s.overrides, vec![("show_poi", SettingValue::Toggle(true))]);
}

/// Built without the feature, the table has no dev setting at all — not
/// merely none offered — so nothing in the module carries its name.
#[cfg(not(feature = "dev"))]
#[test]
fn a_module_without_the_feature_has_no_dev_setting_in_its_table() {
    let _g = serial();
    assert!(crate::shell::setting::schema::SETTINGS.iter().all(|s| !s.dev));
    let mut s = store_under("development");
    assert_eq!(s.get("show_poi"), None);
    s.set_setting("show_poi", SettingValue::Toggle(false));
    assert!(!s.show_poi);
}

/// What a dev setting does, every build does at its default: the POI layer
/// retains its default even when omitted from the settings table.
#[test]
fn a_dev_setting_runs_at_its_default_whether_or_not_it_is_in_the_table() {
    let _g = serial();
    let s = store_under("production");
    assert_eq!(s.show_poi, S::show_poi::default);
    assert_eq!(s.ground_snap, S::ground_snap::default);
    assert_eq!(s.camera_terrain, CameraTerrain::of(S::camera_terrain::default));
}

/// `defaults` is written by hand from the spec's constants, and the table
/// reads the same constants; on either device the two must say the same
/// thing for every setting the table has, or a default would depend on which
/// of them a reader asked.
#[test]
fn the_defaults_are_the_table_s_on_either_device() {
    let _g = serial();
    // Gathered rather than asserted in the loop: the device is a global the
    // other tests read at its default, and an assert that fired mid-loop
    // would leave it handheld for whichever ran next.
    let mut off = Vec::new();
    for handheld in [false, true] {
        set_handheld(handheld);
        let s = store_under("development");
        for (key, value) in DEFAULT_SETTINGS() {
            if s.get(key) != Some(value.clone()) {
                off.push(format!("{key} on handheld={handheld}: {:?} against the table's {value:?}", s.get(key)));
            }
        }
    }
    set_handheld(false);
    assert!(off.is_empty(), "{}", off.join("\n"));
}
