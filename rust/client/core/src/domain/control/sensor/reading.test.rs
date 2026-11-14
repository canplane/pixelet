// pxl-core/domain/control/sensor/reading.test.rs

use super::*;

/// A heading changes only past 1.5° of the one held, measured on the circle,
/// and a heading that appears or goes always changes.
#[test]
fn a_heading_changes_past_its_dead_zone() {
    let mut s = DeviceOrientationStore::default();
    assert!(s.set_orient(Some(359.0)));
    assert_eq!(s.status, SensorStatus::Ready);
    for (to, changed) in [
        (Some(0.4), false),
        (Some(0.6), true),
        (Some(359.2), false),
        (Some(359.0), true),
        (None, true),
        (None, false),
        (Some(10.0), true),
    ] {
        let prev = s.orient.and_then(|o| o.heading);
        assert_eq!(s.set_orient(to), changed, "{prev:?} to {to:?}");
        let cur = if changed { to } else { prev };
        assert_eq!(s.orient.and_then(|o| o.heading), cur);
    }
}

/// A fix moves the location only past about 1.1 m across, 0.00001°, or
/// 0.5 m in altitude.
#[test]
fn a_fix_moves_past_its_dead_zone() {
    let mut s = DeviceLocationStore::default();
    let at = [127.0, 37.5];
    assert!(s.set_location(at, Some(10.0), 5.0, None));
    let near = [at[0] + 0.000009, at[1] - 0.000009];
    assert!(!s.set_location(near, Some(10.4), 5.0, None));
    assert_eq!(
        s.location.map(|l| (l.lon_lat, l.alt)),
        Some((at, Some(10.0)))
    );
    for to in [
        ([at[0] + 0.000011, at[1]], Some(10.0)),
        ([at[0] + 0.000011, at[1] - 0.000011], Some(10.0)),
        ([at[0] + 0.000011, at[1] - 0.000011], Some(10.6)),
        ([at[0] + 0.000011, at[1] - 0.000011], None),
    ] {
        assert!(s.set_location(to.0, to.1, 5.0, None), "{to:?}");
        assert_eq!(s.location.map(|l| (l.lon_lat, l.alt)), Some(to));
    }
}
