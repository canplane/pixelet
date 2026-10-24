// pxl-kernel/setting/store.test.rs

use super::*;

fn kept_of(pairs: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(number, value) in pairs {
        leb(&mut bytes, number);
        leb(&mut bytes, value);
    }
    bytes
}

/// The stored form is a contract with storage, which outlives a build.
#[test]
fn what_is_kept_reads_back_and_a_default_is_not_kept() {
    let mut s = Settings::DEFAULT;
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let ao = AoMode::ALL.into_iter().map(|o| o as u32).find(|&o| o != Settings::DEFAULT.get(SettingKey::ao)).expect("a second option");
    // Past 127, so its LEB128 takes two bytes.
    let person = Settings::DEFAULT.get(SettingKey::person) + 200;
    assert!(s.set(SettingKey::tilt_lock, tilt) && s.set(SettingKey::ao, ao) && s.set(SettingKey::person, person));
    let kept = s.kept();
    let back = read_kept(&kept);
    for key in SettingKey::ALL {
        assert_eq!(back.get(key), s.get(key), "{key:?} read back from {kept:?}");
    }
    assert!(Settings::DEFAULT.kept().is_empty(), "the defaults keep nothing");
}

#[test]
fn a_kept_pair_is_a_setting_s_number_and_its_value() {
    let ao = AoMode::ALL.into_iter().find(|&o| o as u32 != Settings::DEFAULT.get(SettingKey::ao)).expect("a second option");
    let mut s = Settings::DEFAULT;
    assert!(s.set(SettingKey::ao, ao as u32));
    assert_eq!(s.kept(), kept_of(&[(SettingKey::ao as u32, ao as u32)]));
}

#[test]
fn what_the_schema_does_not_hold_is_left_at_the_default() {
    let unknown = SettingKey::SLOTS as u32 + 1000;
    for kept in [
        kept_of(&[(SettingKey::ao as u32, 99)]),
        kept_of(&[(SettingKey::tilt_lock as u32, 2)]),
        kept_of(&[(unknown, 1)]),
        // A pair cut short.
        vec![SettingKey::tilt_lock as u8],
        vec![0x80],
    ] {
        let back = read_kept(&kept);
        for key in SettingKey::ALL {
            assert_eq!(back.get(key), Settings::DEFAULT.get(key), "{kept:?}");
        }
    }
}

#[test]
fn a_pair_past_one_the_schema_does_not_hold_still_reads() {
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let back = read_kept(&kept_of(&[(SettingKey::SLOTS as u32 + 1000, 1), (SettingKey::ao as u32, 99), (SettingKey::tilt_lock as u32, tilt)]));
    assert_eq!(back.get(SettingKey::tilt_lock), tilt);
    assert_eq!(back.get(SettingKey::ao), Settings::DEFAULT.get(SettingKey::ao));
}

/// The JSON by name the slot held before is not of its format: the settings
/// start at the defaults, and nothing is written over it until it is emptied.
#[test]
fn a_slot_of_another_format_is_refused_and_held_unwritten() {
    let _g = crate::serial();
    file::emptied(Kept::Settings);
    settings_load(r#"{"ao":"screen"}"#);
    assert_eq!(*settings_store(), Settings::DEFAULT);
    assert!(file::refused(Kept::Settings));
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    assert_eq!(settings_store().set_setting(SettingKey::tilt_lock, tilt), Some(true));
    settings_keep_step();
    assert!(*UNKEPT.get(|| false), "a refused slot's change waits for it to be emptied");
    file::emptied(Kept::Settings);
    settings_keep_step();
    assert!(!*UNKEPT.get(|| false));
}
