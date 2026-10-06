// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

pub mod setting;

use crate::world::pyramid::Observed as _;
use pxl_spec::bake::place_tile::leaf_slot;
use pxl_spec::cell::object::{decor_num_id, placeable_cube_num_id};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use xpute_core::wire::xtp::PacketWriter;

pub use crate::abi::cmd::Tracking;
use crate::abi::cmd::{CompanionState, HoldingState, LocationState, LonLat, MapState, MapStatus, OrientationState, PlaceName, PlacePart, Snapshot};
use crate::control::camera::controller::{dist, map_center, units_per_px_at, Motion};
use crate::control::camera::store::{camera_store, CameraStatus, IntentPatch};
use crate::control::interaction::holding;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::device_orientation::{orientation_close, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::store::{device_location_store, device_orientation_store};
use crate::layer::geocode::fetcher::center_group;
use crate::layer::geocode::store as geocode_store;
use crate::scene::character::companion::companion_load_store;
use crate::scene::character::preview::{preview_rows, CASES_MAX};
use crate::sched::turn::world_platform;
use crate::shell::setting::store::{settings_store, SettingKey};
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// The center tile's place, broad to narrow. Every language is sent, so
/// switching language re-reads rather than refetches.
fn place_parts(tile: Option<IVec>, each: &mut dyn FnMut(&PlacePart<'_>)) {
    let parts = tile
        .zip(center_group())
        .zip(geocode_store::dict())
        .and_then(|((tile, group), dict)| {
            let group = geocode_store::group_of(group)?;
            group.view.place_of(leaf_slot(ivec::X(tile), ivec::Y(tile)), &dict.view)
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
    let location = device_location_store();
    let orientation = device_orientation_store();
    let camera = camera_store();
    let settings = settings_store();
    let companion = companion_load_store();
    let at = crate::control::camera::controller::map_center();
    let tile = at.map(|c| idx(LEAF_ZOOM, c.i));
    let center = at.map(|c| {
        let g = epsg4326::unproject(c.i, c.f);
        LonLat { lon: g[0], lat: g[1] }
    });
    let values = SettingKey::ALL.map(|key| settings.get(key));
    let held = holding();
    let mut previews = [0u32; CASES_MAX];
    let mut preview_cnt = 0;
    for (slot, (surface, _, _)) in previews.iter_mut().zip(preview_rows()) {
        *slot = surface;
        preview_cnt += 1;
    }
    let snapshot = Snapshot {
        map: MapState {
            // Loaded once ground is drawn, or every tile the view needs has answered,
            // which over a source's void leaves nothing to draw.
            status: match crate::world::view::observation::eye() {
                None => MapStatus::Idle,
                Some(_) if crate::kernel().sched.shows_ground() || Terrain::pyramid().answered() => MapStatus::Success,
                Some(_) => MapStatus::Loading,
            },
            is_off_center: map_navigation().is_off_center,
            center,
            outside: at.is_some_and(|c| void_under(&c)),
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
        holding: HoldingState {
            tool: held.active_tool,
            cube: placeable_cube_num_id(held.object_cube),
            decor: decor_num_id(held.decor_id),
            tier: held.object_tier,
        },
        orientation: OrientationState {
            status: orientation.pose.status(),
            error: orientation.pose.error(),
        },
        azimuth: camera.intent.azimuth,
        is_follow_mode: camera.is_follow_mode,
        camera_busy: camera.camera_status != CameraStatus::Idle,
        zooming: crate::control::camera::controller::zooming(),
        settings: &values,
        m_per_px: m_per_px(),
        place: &|each| place_parts(tile, each),
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

/// A badge's height is its zoom's, coarser than where the center stands, so
/// the center goes on the surface once it is known exactly.
fn go_on_ground(target: Pvf3, motion: Motion) {
    camera_store().is_follow_mode = false;
    go_to_ground(target, motion);
}

/// A place, as the location button goes to one: a view far out comes in.
pub fn go_to(target: Pvf3) {
    go_on_ground(target, Motion::Smooth);
    crate::control::camera::controller::come_in_to_default();
}

/// Nearer where the camera has a nearer to be: the orbit halves its distance.
pub fn go_into(target: Pvf3) {
    go_on_ground(target, Motion::Smooth);
    if crate::control::camera::controller::active() == crate::control::camera::controller::Active::Orbit {
        crate::control::camera::orbit::zoom_by(0.5);
    }
}

/// A search's place, as the location button goes to one.
pub fn go_to_degrees(lon: f64, lat: f64) {
    go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
    crate::control::camera::controller::come_in_to_default();
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

/// The view's scale at the map center, as the meters a logical pixel spans;
/// 0 where the camera stands at the center: no distance, no scale.
fn m_per_px() -> f64 {
    let y_u = map_center().map_or(0.0, |c| pvf3::to_real(pvf3::Y(&c)));
    units_per_px_at(dist()) / epsg3857::u_per_m(y_u)
}

/// The source has nothing under `at`: the ground cell there, as the finest
/// tile held over it gives it, is void.
fn void_under(at: &Pvf3) -> bool {
    crate::world::query::Access::now().at(at, LEAF_ZOOM).value().is_some_and(|g| g.top().is_none())
}
