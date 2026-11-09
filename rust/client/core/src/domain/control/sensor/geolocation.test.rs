// pxl-core/domain/control/sensor/geolocation.test.rs

use super::*;
use crate::sys::serial;

/// A fix is kept where the device put it. Only its altitude is read at the
/// leaf tile's corner: over sea level by the geoid there, which lies within
/// a hair of the geoid at the fix.
#[test]
fn a_fix_is_kept_where_it_is() {
    let _g = serial();
    device_location_store_mut().location = None;
    let (lon, lat) = (126.9784, 37.5667);
    geolocation_fix(lon, lat, Some(100.0), 5.0, None);
    let got = device_location_store().location.expect("a fix");
    assert_eq!(got.lon_lat, [lon, lat]);
    let alt = got.alt.expect("an altitude");
    let want = 100.0 - epsg4326::geoid_m(lon, lat);
    assert!((alt - want).abs() < 0.05, "{alt} against {want}");
}
