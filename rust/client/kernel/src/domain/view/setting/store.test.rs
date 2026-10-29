// pxl-kernel/domain/view/setting/store.test.rs

use super::*;
use crate::sys;
use crate::sys::io::host;
use base64::engine::general_purpose;
use xpute_kit::status::errno::Errno;

fn kept_of(pairs: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(number, val) in pairs {
        leb(&mut bytes, number);
        leb(&mut bytes, val);
    }
    bytes
}

/// The stored form is a contract with storage, which outlives a build.
#[test]
fn kept_round_trip() {
    let mut s = Settings::DEFAULT;
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let ao = AoMode::ALL
        .into_iter()
        .map(|o| o as u32)
        .find(|&o| o != Settings::DEFAULT.get(SettingKey::ao))
        .expect("a second option");
    // Past 127, so its LEB128 takes two bytes.
    let person = Settings::DEFAULT.get(SettingKey::person) + 200;
    assert!(
        s.set(SettingKey::tilt_lock, tilt)
            && s.set(SettingKey::ao, ao)
            && s.set(SettingKey::person, person)
    );
    let kept = s.kept();
    let back = read_kept(&kept);
    for key in SettingKey::ALL {
        assert_eq!(back.get(key), s.get(key), "{key:?} read back from {kept:?}");
    }
    assert!(
        Settings::DEFAULT.kept().is_empty(),
        "the defaults keep nothing"
    );
}

#[test]
fn kept_pair_format() {
    let ao = AoMode::ALL
        .into_iter()
        .find(|&o| o as u32 != Settings::DEFAULT.get(SettingKey::ao))
        .expect("a second option");
    let mut s = Settings::DEFAULT;
    assert!(s.set(SettingKey::ao, ao as u32));
    assert_eq!(s.kept(), kept_of(&[(SettingKey::ao as u32, ao as u32)]));
}

#[test]
fn unknown_stays_default() {
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
fn reads_past_unknown_pair() {
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let back = read_kept(&kept_of(&[
        (SettingKey::SLOTS as u32 + 1000, 1),
        (SettingKey::ao as u32, 99),
        (SettingKey::tilt_lock as u32, tilt),
    ]));
    assert_eq!(back.get(SettingKey::tilt_lock), tilt);
    assert_eq!(
        back.get(SettingKey::ao),
        Settings::DEFAULT.get(SettingKey::ao)
    );
}

/// A setting schema.json marks `kept` false is the session's: it is not in
/// what is kept, a change to it alone asks for no write, and one found in
/// the slot reads as its default. A dev one this build does not offer
/// takes no change at all (`is_offered`), so only the offered are changed.
#[test]
fn session_setting_not_kept() {
    let _g = sys::serial();
    let session: Vec<SettingKey> = SettingKey::ALL
        .into_iter()
        .filter(|k| !k.is_kept())
        .collect();
    assert!(
        session.contains(&SettingKey::place) && session.contains(&SettingKey::camera_mode),
        "places shown and the camera mode are the session's"
    );
    let other = |key: SettingKey| match key.kind() {
        SettingKind::Choice => (0..=u32::from(u8::MAX))
            .find(|&v| v != Settings::DEFAULT.get(key) && Settings::DEFAULT.clone().set(key, v))
            .expect("a second option"),
        _ => 1 - Settings::DEFAULT.get(key),
    };
    let mut s = Settings::DEFAULT;
    for &key in &session {
        assert!(s.set(key, other(key)));
    }
    assert!(s.kept().is_empty(), "nothing of {session:?} is kept");
    let back = read_kept(&kept_of(
        &session
            .iter()
            .map(|&key| (key as u32, other(key)))
            .collect::<Vec<_>>(),
    ));
    assert_eq!(back, Settings::DEFAULT, "nothing of {session:?} is read");
    open_slot();
    file::on_kept_answered(Kept::Settings, Err(file::Opened::Absent));
    assert!(settings_load_step());
    for &key in session.iter().filter(|&&k| is_offered(k, other(k))) {
        assert_eq!(settings_store().set_setting(key, other(key)), Some(true));
    }
    assert!(
        !*UNKEPT.get(|| false),
        "a session's change asks for no write"
    );
}

/// What storage holds of the slot, as the host hands it over.
fn file_of(pairs: &[(u32, u32)]) -> Vec<u8> {
    use base64::Engine;
    let mut bytes = vec![Kept::Settings.format()];
    bytes.extend(kept_of(pairs));
    general_purpose::URL_SAFE_NO_PAD.encode(bytes).into_bytes()
}

/// The slot opened at boot, with nothing waiting and nothing to keep.
fn open_slot() {
    file::on_emptied(Kept::Settings);
    while file::take_kept(Kept::Settings).is_some() {}
    settings_open();
    UNKEPT.set(false);
}

/// One change, then a keep: whether it was held, unwritten.
fn held_after_a_change() -> bool {
    let tilt = 1 - settings_store().get(SettingKey::tilt_lock);
    assert_eq!(
        settings_store().set_setting(SettingKey::tilt_lock, tilt),
        Some(true)
    );
    settings_keep_step();
    *UNKEPT.get(|| false)
}

/// The slot is read on the storage root a turn after its file lands, and
/// nothing is written before; what was read is not written back.
#[test]
fn read_before_write() {
    let _g = sys::serial();
    open_slot();
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    assert!(
        !settings_load_step() && !are_settings_loaded(),
        "nothing has landed"
    );
    assert!(
        held_after_a_change(),
        "nothing is written before the slot is read"
    );
    file::on_kept_answered(
        Kept::Settings,
        Ok(&file_of(&[(SettingKey::tilt_lock as u32, tilt)])),
    );
    assert!(!settings_load_step(), "taken from the next turn on");
    host::rise();
    assert!(settings_load_step() && are_settings_loaded());
    assert_eq!(settings_store().get(SettingKey::tilt_lock), tilt);
    assert!(!*UNKEPT.get(|| false), "what was read is not written back");
    assert!(!held_after_a_change(), "a read slot keeps what changes");
    assert!(!settings_load_step(), "read once");
}

/// An empty slot is the defaults, written from the first change.
#[test]
fn empty_slot_defaults() {
    let _g = sys::serial();
    open_slot();
    file::on_kept_answered(Kept::Settings, Err(file::Opened::Absent));
    assert!(settings_load_step());
    assert_eq!(*settings_store(), Settings::DEFAULT);
    assert!(!held_after_a_change());
}

/// A file too large for any heap, or of another format (the JSON by name the
/// slot held before), is refused: the defaults, nothing written over it until
/// the host sets it aside and empties the slot, and then what changed meanwhile.
#[test]
fn bad_slot_refused() {
    let _g = sys::serial();
    for answer in [
        Err(file::Opened::Failed {
            errnum: Errno::EFBIG,
            detail: 0,
        }),
        Ok(&br#"{"ao":"screen"}"#[..]),
    ] {
        open_slot();
        file::on_kept_answered(Kept::Settings, answer);
        host::rise();
        assert!(
            settings_load_step() && are_settings_loaded(),
            "the world need not wait on it"
        );
        assert_eq!(*settings_store(), Settings::DEFAULT);
        assert!(file::is_refused(Kept::Settings));
        assert!(
            held_after_a_change(),
            "a refused slot's change waits for it to be emptied"
        );
        file::on_emptied(Kept::Settings);
        on_settings_emptied();
        settings_keep_step();
        assert!(!*UNKEPT.get(|| false));
    }
}

/// One that fails to open otherwise is not the host's to set aside: the
/// defaults, never written over this session.
#[test]
fn unreadable_slot_kept() {
    let _g = sys::serial();
    open_slot();
    file::on_kept_answered(
        Kept::Settings,
        Err(file::Opened::Failed {
            errnum: Errno::EIO,
            detail: 0,
        }),
    );
    assert!(settings_load_step());
    assert!(!file::is_refused(Kept::Settings));
    assert!(held_after_a_change());
    on_settings_emptied();
    assert!(held_after_a_change());
}
