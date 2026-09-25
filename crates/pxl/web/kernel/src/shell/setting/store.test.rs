// pxl-kernel/shell/setting/store.test.rs

use super::*;

/// What differs from the defaults is stored by name and read back; a value
/// set back to its default leaves storage.
#[test]
fn what_is_stored_is_what_differs_and_it_is_read_back() {
    let mut s = Settings::DEFAULT;
    s.set(SettingKey::ao, AoMode::screen as u32);
    s.set(SettingKey::person_variant, 3);
    let raw = stored(&s);
    assert_eq!(raw, r#"{"ao":"screen","person_variant":3}"#);
    let back = load(&raw);
    assert_eq!(back.ao, AoMode::screen);
    assert_eq!(back.person_variant, 3);
    s.set(SettingKey::ao, Settings::DEFAULT.ao as u32);
    assert_eq!(stored(&s), r#"{"person_variant":3}"#);
}
