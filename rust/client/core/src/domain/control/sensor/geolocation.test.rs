// pxl-core/domain/control/sensor/geolocation.test.rs

use super::*;

/// A fix is kept where the device put it. Only its altitude is read at the
/// leaf tile's corner: over sea level by the geoid there, which lies within
/// a hair of the geoid at the fix.
#[test]
fn a_fix_is_kept_where_it_is() {
    crate::on_core(|| {
        device_location_store_mut().location = None;
        let (lon, lat) = (126.9784, 37.5667);
        geolocation_fix(lon, lat, Some(100.0), 5.0, None);
        let got = device_location_store().location.expect("a fix");
        assert_eq!(got.lon_lat, [lon, lat]);
        let alt = got.alt.expect("an altitude");
        let want = 100.0 - epsg4326::geoid_m(lon, lat);
        assert!((alt - want).abs() < 0.05, "{alt} against {want}");
    });
}

/// Opens the location afresh on a platform that has one.
fn opened() {
    platform().supported = true;
    geolocation_close();
    take_geolocation_cmd();
    geolocation_open();
    assert_eq!(take_geolocation_cmd(), Some(GeolocationCommand::Start));
}

/// A failure the watch rides out leaves it running, so asking again starts
/// nothing; one that ended it leaves nothing running and no location, so
/// asking again starts a watch.
#[test]
fn the_platform_says_whether_the_watch_runs_on() {
    crate::on_core(|| {
        opened();
        geolocation_fix(126.9784, 37.5667, None, 5.0, None);
        geolocation_err(GeolocationErrorCode::PositionUnavailable, true);
        let store = device_location_store();
        assert!(store.is_tracking_location);
        assert!(store.location.is_some());
        start_watching();
        assert_eq!(take_geolocation_cmd(), None);

        geolocation_err(GeolocationErrorCode::PositionUnavailable, false);
        let store = device_location_store();
        assert!(!store.is_tracking_location);
        assert!(store.location.is_none());
        assert_eq!(store.state.status, SensorStatus::Error);
        start_watching();
        assert_eq!(take_geolocation_cmd(), Some(GeolocationCommand::Start));
        assert_eq!(device_location_store().state.status, SensorStatus::Loading);
    });
}

/// Hiding the app stops the watch and keeps what it read: on the return the
/// location is still known, and reads as Loading until a fix of the new
/// watch.
#[test]
fn a_hidden_spell_keeps_the_location() {
    crate::on_core(|| {
        opened();
        geolocation_fix(126.9784, 37.5667, None, 5.0, None);
        geolocation_visibility(false);
        assert_eq!(take_geolocation_cmd(), Some(GeolocationCommand::Stop));
        assert!(device_location_store().location.is_some());
        geolocation_visibility(true);
        assert_eq!(take_geolocation_cmd(), Some(GeolocationCommand::Start));
        let store = device_location_store();
        assert!(store.location.is_some());
        assert_eq!(store.state.status, SensorStatus::Loading);

        geolocation_close();
        let store = device_location_store();
        assert!(store.location.is_none());
        assert_eq!(store.state.status, SensorStatus::Idle);
    });
}
