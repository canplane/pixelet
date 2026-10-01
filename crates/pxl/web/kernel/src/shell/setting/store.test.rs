// pxl-kernel/shell/setting/store.test.rs

use super::*;

/// What storage keeps reads back as what was kept: a toggle, a choice and an
/// index set off their defaults, by name, and nothing for what stands at its
/// default — the stored form is a contract with storage, which outlives a
/// build.
#[test]
fn what_is_kept_reads_back_and_a_default_is_not_kept() {
    let mut s = Settings::DEFAULT;
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let ao = (Settings::DEFAULT.get(SettingKey::ao) + 1) % SettingKey::ao.options().len() as u32;
    let person = Settings::DEFAULT.get(SettingKey::person) + 2;
    assert!(s.set(SettingKey::tilt_lock, tilt) && s.set(SettingKey::ao, ao) && s.set(SettingKey::person, person));
    let kept = s.kept();
    let back = read_kept(&kept);
    for key in SettingKey::ALL {
        assert_eq!(back.get(key), s.get(key), "{} read back from {kept}", key.name());
    }
    assert_eq!(Settings::DEFAULT.kept(), "{}", "the defaults keep nothing");
}

/// A stored value the schema has no place for is left at the default: an
/// option a choice does not have, a name it does not know, storage that is
/// not JSON.
#[test]
fn what_the_schema_does_not_hold_is_left_at_the_default() {
    for kept in [r#"{"ao":"nonesuch"}"#, r#"{"nonesuch":true}"#, "not json"] {
        let back = read_kept(kept);
        for key in SettingKey::ALL {
            assert_eq!(back.get(key), Settings::DEFAULT.get(key), "{kept}");
        }
    }
}
