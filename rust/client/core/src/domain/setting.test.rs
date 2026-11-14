// pxl-core/domain/setting.test.rs

use super::*;

fn storage_of(pairs: &[(u32, u32)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(number, val) in pairs {
        leb(&mut bytes, number);
        leb(&mut bytes, val);
    }
    bytes
}

/// The stored form is a contract with storage, which outlives a build.
#[test]
fn stored_round_trip() {
    crate::on_core(|| {
        let mut s = Setting::DEFAULT;
        let tilt = 1 - Setting::DEFAULT.get(SettingKey::TiltLock);
        let ao = AoMode::ALL
            .into_iter()
            .map(|o| o as u32)
            .find(|&o| o != Setting::DEFAULT.get(SettingKey::Ao))
            .expect("a second option");
        // The value is past 127, so its LEB128 takes two bytes.
        let person = Setting::DEFAULT.get(SettingKey::Person) + 200;
        assert!(
            s.set(SettingKey::TiltLock, tilt)
                && s.set(SettingKey::Ao, ao)
                && s.set(SettingKey::Person, person)
        );
        let bytes = s.storage_bytes();
        let back = read_stored(&bytes);
        for key in SettingKey::ALL {
            assert_eq!(
                back.get(key),
                s.get(key),
                "{key:?} read back from {bytes:?}"
            );
        }
        assert!(
            Setting::DEFAULT.storage_bytes().is_empty(),
            "the defaults keep nothing"
        );
    });
}

#[test]
fn stored_pair_format() {
    crate::on_core(|| {
        let ao = AoMode::ALL
            .into_iter()
            .find(|&o| o as u32 != Setting::DEFAULT.get(SettingKey::Ao))
            .expect("a second option");
        let mut s = Setting::DEFAULT;
        assert!(s.set(SettingKey::Ao, ao as u32));
        assert_eq!(
            s.storage_bytes(),
            storage_of(&[(SettingKey::Ao as u32, ao as u32)])
        );
    });
}

#[test]
fn unknown_stays_default() {
    crate::on_core(|| {
        let unknown = SettingKey::SLOT_CNT as u32 + 1000;
        for bytes in [
            storage_of(&[(SettingKey::Ao as u32, 99)]),
            storage_of(&[(SettingKey::TiltLock as u32, 2)]),
            storage_of(&[(unknown, 1)]),
            // A pair cut short after its number, and a number cut short.
            vec![SettingKey::TiltLock as u8],
            vec![0x80],
        ] {
            let back = read_stored(&bytes);
            for key in SettingKey::ALL {
                assert_eq!(back.get(key), Setting::DEFAULT.get(key), "{bytes:?}");
            }
        }
    });
}

#[test]
fn reads_past_unknown_pair() {
    crate::on_core(|| {
        let tilt = 1 - Setting::DEFAULT.get(SettingKey::TiltLock);
        let back = read_stored(&storage_of(&[
            (SettingKey::SLOT_CNT as u32 + 1000, 1),
            (SettingKey::Ao as u32, 99),
            (SettingKey::TiltLock as u32, tilt),
        ]));
        assert_eq!(back.get(SettingKey::TiltLock), tilt);
        assert_eq!(
            back.get(SettingKey::Ao),
            Setting::DEFAULT.get(SettingKey::Ao)
        );
    });
}
