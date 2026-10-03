// pxl-kernel/shell/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

use pxl_spec::bake::place_page::leaf_slot;
use pxl_spec::space::adapter::epsg4326;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::wire::xtp::PacketWriter;

pub use crate::abi::cmd::Tracking;
use crate::abi::cmd::{CompanionState, LocationState, LonLat, MapState, OrientationState, PlaceName, PlacePart, Snapshot};
use crate::control::camera::controller::Motion;
use crate::control::camera::store::{camera_store, CameraStatus, IntentPatch};
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::device_orientation::{orientation_close, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::store::{device_location_store, device_orientation_store};
use crate::layer::character::preview::{preview_rows, CASES_MAX};
use crate::layer::character::store::companion_load_store;
use crate::layer::geocode::fetcher::center_group;
use crate::layer::geocode::store as geocode_store;
use crate::sched::turn::world_platform;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, SettingKey};
use crate::shell::ui::scale::m_per_px;

/// The center page's place, broad to narrow. Every language is sent, so
/// switching language re-reads rather than refetches.
fn place_parts(page: Option<IVec>, each: &mut dyn FnMut(&PlacePart<'_>)) {
    let parts = page
        .zip(center_group())
        .zip(geocode_store::dict())
        .and_then(|((page, group), dict)| {
            let group = geocode_store::group_of(group)?;
            group.view.place_of(leaf_slot(ivec::X(page), ivec::Y(page)), &dict.view)
        })
        .into_iter()
        .flatten();
    for part in parts {
        let names = |each: &mut dyn FnMut(&PlaceName<'_>)| {
            for (lang, name) in part.languages() {
                each(&PlaceName { lang: &lang, name: &name });
            }
        };
        each(&PlacePart {
            rank: part.rank as u8,
            name: &part.name,
            names: &names,
        });
    }
}

/// None where it does not fit.
pub fn ui_snapshot(out: &mut [u8]) -> Option<u32> {
    let map = map_store();
    let location = device_location_store();
    let orientation = device_orientation_store();
    let camera = camera_store();
    let settings = settings_store();
    let companion = companion_load_store();
    let at = crate::control::camera::controller::map_center();
    let page = at.map(|c| L3_IDX(c.i));
    let center = at.map(|c| {
        let g = epsg4326::unproject(c.i, c.f);
        LonLat { lon: g[0], lat: g[1] }
    });
    let values = SettingKey::ALL.map(|key| settings.get(key));
    let mut previews = [0u32; CASES_MAX];
    let mut preview_cnt = 0;
    for (slot, (surface, _, _)) in previews.iter_mut().zip(preview_rows()) {
        *slot = surface;
        preview_cnt += 1;
    }
    let snapshot = Snapshot {
        map: MapState {
            status: map.map_status,
            is_off_center: map_navigation().is_off_center,
            center,
        },
        location: LocationState {
            status: location.pose.status(),
            has_location: location.location.is_some(),
            error: location.pose.error(),
        },
        companion: CompanionState {
            person_ready: companion.person_ready,
            pet_ready: companion.pet_ready,
        },
        orientation: OrientationState { status: orientation.pose.status() },
        azimuth: camera.intent.azimuth,
        is_follow_mode: camera.is_follow_mode,
        camera_busy: camera.camera_status != CameraStatus::Idle,
        zooming: crate::control::camera::controller::zooming(),
        settings: &values,
        m_per_px: m_per_px(),
        place: &|each| place_parts(page, each),
        previews: &previews[..preview_cnt],
    };
    let mut w = PacketWriter::new(out, Snapshot::FIELDS);
    snapshot.write(&mut w);
    w.finish()
}

/// The shell decides which of the three from the state it drew the button
/// in, so the press means what the user saw.
pub fn tracking_action(tracking: Tracking) {
    let camera = camera_store();
    match tracking {
        Tracking::Recenter => {
            // A fix's Z is altitude, absent on most and never the map's ground height.
            if let Some(device) = map_navigation().device_coord {
                go_to_ground(device, Motion::Snap);
            }
            camera.is_follow_mode = false;
            // A view far out comes back in; one brought nearer stays, as map apps do.
            crate::control::camera::controller::come_in_to_default();
        }
        Tracking::Follow => {
            // The heading is asked for, and the map turns with it once it comes;
            // where there is none, following is where one is alone.
            request_orientation(world_platform().orientation);
            camera.is_follow_mode = true;
        }
        Tracking::Unfollow => {
            camera.is_follow_mode = false;
            crate::control::camera::controller::reset_azimuth();
        }
    }
}

/// A badge's height is its level's, coarser than where the center stands, so
/// the center goes on the surface once it is known exactly.
pub fn go_to(target: Pvf3) {
    camera_store().is_follow_mode = false;
    go_to_ground(target, Motion::Smooth);
}

/// `go_to`, and nearer where the camera has a nearer to be: the orbit halves its distance.
pub fn go_into(target: Pvf3) {
    go_to(target);
    if crate::control::camera::controller::active() == crate::control::camera::controller::Active::Orbit {
        crate::control::camera::orbit::zoom_by(0.5);
    }
}

pub fn go_to_degrees(lon: f64, lat: f64) {
    camera_store().is_follow_mode = false;
    let target = epsg4326::project(&[lon, lat, 0.0]);
    go_to_ground(target, Motion::Snap);
}

/// Set outright, for kernel tests and A/B comparisons at the same view.
pub fn set_view(dist: Option<f64>, polar: Option<f64>) {
    crate::control::camera::controller::set_intent(IntentPatch { dist, azimuth: None, polar });
}

/// The compass is asked for where it is used (`tracking_action`).
pub fn location_enable() {
    start_watching(world_platform().geolocation);
}

pub fn sensors_open() {
    let platform = world_platform();
    geolocation_open(platform.geolocation);
    request_orientation(platform.orientation);
}

pub fn sensors_close() {
    geolocation_close();
    orientation_close();
}
