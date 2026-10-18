// pxl-kernel/setting/store.test.rs

use super::*;

/// The stored form is a contract with storage, which outlives a build.
#[test]
fn what_is_kept_reads_back_and_a_default_is_not_kept() {
    let mut s = Settings::DEFAULT;
    let tilt = 1 - Settings::DEFAULT.get(SettingKey::tilt_lock);
    let person = Settings::DEFAULT.get(SettingKey::person) + 2;
    assert!(s.set(SettingKey::tilt_lock, tilt) && s.set(SettingKey::orbit_obstacle, OrbitObstacleMode::stop as u32) && s.set(SettingKey::person, person));
    let kept = s.kept();
    let back = read_kept(&kept);
    for key in SettingKey::ALL {
        assert_eq!(back.get(key), s.get(key), "{} read back from {kept}", key.name());
    }
    assert_eq!(Settings::DEFAULT.kept(), "{}", "the defaults keep nothing");
}

#[test]
fn what_the_schema_does_not_hold_is_left_at_the_default() {
    for kept in [r#"{"orbit_obstacle":"nonesuch"}"#, r#"{"nonesuch":true}"#, "not json"] {
        let back = read_kept(kept);
        for key in SettingKey::ALL {
            assert_eq!(back.get(key), Settings::DEFAULT.get(key), "{kept}");
        }
    }
}
