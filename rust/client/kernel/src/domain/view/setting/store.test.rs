// pxl-kernel/domain/view/setting/store.test.rs

use super::*;
use xpute_kit::status::errno::Errno;

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
    let ao = AoMode::ALL
        .into_iter()
        .map(|o| o as u32)
        .find(|&o| o != Settings::DEFAULT.get(SettingKey::ao))
        .expect("a second option");
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

/// What storage holds of the slot, as the host hands it over.
fn file_of(pairs: &[(u32, u32)]) -> Vec<u8> {
    use base64::Engine;
    let mut bytes = vec![Kept::Settings.format()];
    bytes.extend(kept_of(pairs));
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes).into_bytes()
}

/// The slot opened at boot, with nothing waiting and nothing to keep.
fn opened() {
    file::emptied(Kept::Settings);
    while file::take_kept(Kept::Settings).is_some() {}
    settings_open();
    UNKEPT.set(false);
}

/// One change, then a keep: whether it was held, unwritten.
fn held_after_a_change() -> bool {
    let tilt = 1 - settings_store().get(SettingKey::tilt_lock);
    assert_eq!(settings_store().set_setting(SettingKey::tilt_lock, tilt), Some(true));
    settings_keep_step();
    *UNKEPT.get(|| false)
}

/// The slot is read on the storage root a turn after its file lands, and
/// nothing is written before; what was read is not written back.
#[test]
fn the_settings_are_read_from_their_slot_and_nothing_is_written_before() {
    let _g = crate::sys::serial();
    opened();
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    assert!(!settings_load_step() && !settings_loaded(), "nothing has landed");
    assert!(held_after_a_change(), "nothing is written before the slot is read");
    file::kept_answered(Kept::Settings, Ok(&file_of(&[(SettingKey::tilt_lock as u32, tilt)])));
    assert!(!settings_load_step(), "taken from the next turn on");
    crate::sys::io::host::host_rise();
    assert!(settings_load_step() && settings_loaded());
    assert_eq!(settings_store().get(SettingKey::tilt_lock), tilt);
    assert!(!*UNKEPT.get(|| false), "what was read is not written back");
    assert!(!held_after_a_change(), "a read slot keeps what changes");
    assert!(!settings_load_step(), "read once");
}

/// An empty slot is the defaults, written from the first change.
#[test]
fn an_empty_slot_is_the_defaults() {
    let _g = crate::sys::serial();
    opened();
    file::kept_answered(Kept::Settings, Err(file::Opened::Absent));
    assert!(settings_load_step());
    assert_eq!(*settings_store(), Settings::DEFAULT);
    assert!(!held_after_a_change());
}

/// A file too large for any heap, or of another format (the JSON by name the
/// slot held before), is refused: the defaults, nothing written over it until
/// the host sets it aside and empties the slot, and then what changed meanwhile.
#[test]
fn an_oversize_or_foreign_slot_is_refused_and_held_unwritten() {
    let _g = crate::sys::serial();
    for answer in [Err(file::Opened::Failed { errno: Errno::EFBIG, detail: 0 }), Ok(&br#"{"ao":"screen"}"#[..])] {
        opened();
        file::kept_answered(Kept::Settings, answer);
        crate::sys::io::host::host_rise();
        assert!(settings_load_step() && settings_loaded(), "the world need not wait on it");
        assert_eq!(*settings_store(), Settings::DEFAULT);
        assert!(file::refused(Kept::Settings));
        assert!(held_after_a_change(), "a refused slot's change waits for it to be emptied");
        file::emptied(Kept::Settings);
        settings_emptied();
        settings_keep_step();
        assert!(!*UNKEPT.get(|| false));
    }
}

/// One that fails to open otherwise is not the host's to set aside: the
/// defaults, never written over this session.
#[test]
fn an_unreadable_slot_is_never_written_over() {
    let _g = crate::sys::serial();
    opened();
    file::kept_answered(Kept::Settings, Err(file::Opened::Failed { errno: Errno::EIO, detail: 0 }));
    assert!(settings_load_step());
    assert!(!file::refused(Kept::Settings));
    assert!(held_after_a_change());
    settings_emptied();
    assert!(held_after_a_change());
}
