// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

use crate::view::demand::Observed as _;
use pxl_spec::bake::place_tile::leaf_slot;
use pxl_spec::cell::object::{decor_num_id, placeable_cube_num_id};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use xpute_core::wire::xtp::PacketWriter;

pub use crate::abi::cmd::Tracking;
use crate::abi::cmd::{CompanionState, FocusState, GroundStatus, HoldingState, LocationState, LonLat, OrientationState, PlaceName, PlacePart, Snapshot};
use crate::control::camera::controller::{dist, focus, Motion};
use crate::control::camera::state::{camera_state, CameraStatus, IntentPatch};
use crate::control::interaction::holding;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::follow_mode::{following, set_following};
use crate::control::navigation::off_center::off_center;
use crate::control::sensor::device_orientation::{orientation_close, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::reading::{device_location_store, device_orientation_store};
use crate::layer::geocode::fetcher::group_of;
use crate::layer::geocode::store as geocode_store;
use crate::scene::character::companion::companion_load_store;
use crate::scene::character::preview::{preview_rows, CASES_MAX};
use crate::setting::store::{settings_store, SettingKey};
use crate::turn::world_platform;
use crate::view::frame::pose::units_per_px_at;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// The center tile's place, broad to narrow. Every language is sent, so
/// switching language re-reads rather than refetches.
fn place_parts(tile: Option<IVec>, each: &mut dyn FnMut(&PlacePart<'_>)) {
    let parts = tile
        .zip(crate::control::camera::controller::focus().map(|c| group_of(&c)))
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
    let camera = camera_state();
    let settings = settings_store();
    let companion = companion_load_store();
    let at = crate::control::camera::controller::focus();
    let tile = at.map(|c| idx(LEAF_ZOOM, c.i));
    let center = at.map(|c| {
        let g = epsg4326::unproject(c.i, c.f);
        LonLat { lon: g[0], lat: g[1] }
    });
    let values = SettingKey::ALL.map(|key| settings.get(key));
    let held = holding();
    let (waiting, words) = crate::control::interaction::edit::marker_waiting();
    let mut previews = [0u32; CASES_MAX];
    let mut preview_cnt = 0;
    for (slot, (surface, _, _)) in previews.iter_mut().zip(preview_rows()) {
        *slot = surface;
        preview_cnt += 1;
    }
    let snapshot = Snapshot {
        focus: FocusState {
            // Loaded once ground is drawn, or every tile the view needs has answered,
            // which over a source's void leaves nothing to draw.
            status: match crate::view::observation::eye() {
                None => GroundStatus::Idle,
                Some(_) if crate::kernel().sched.shows_ground() || Terrain::demand().answered() => GroundStatus::Success,
                Some(_) => GroundStatus::Loading,
            },
            is_off_center: off_center().is_off_center,
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
            marker: waiting,
            words: &words,
        },
        orientation: OrientationState {
            status: orientation.pose.status(),
            error: orientation.pose.error(),
        },
        azimuth: camera.intent.azimuth,
        is_follow_mode: following(),
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
    match tracking {
        Tracking::Recenter => {
            // A fix's Z is altitude, absent on most and never the ground's height.
            if let Some(device) = off_center().device_coord {
                go_to_ground(device, Motion::Snap);
            }
            set_following(false);
            crate::control::camera::controller::come_in_to_default();
        }
        Tracking::Follow => {
            // The heading is asked for, and the view turns with it once it comes;
            // where there is none, following is where one is alone.
            request_orientation(world_platform().orientation);
            set_following(true);
        }
        Tracking::Unfollow => {
            set_following(false);
            crate::control::camera::controller::reset_azimuth();
        }
    }
}

/// A search's place, as the location button goes to one.
pub fn go_to_degrees(lon: f64, lat: f64) {
    crate::control::navigation::destination::go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
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

/// The view's scale at the focus, as the meters a logical pixel spans;
/// 0 where the camera stands at the center: no distance, no scale.
fn m_per_px() -> f64 {
    let y_u = focus().map_or(0.0, |c| pvf3::to_real(pvf3::Y(&c)));
    units_per_px_at(dist()) / epsg3857::u_per_m(y_u)
}

/// The source has nothing under `at`: the ground cell there, as the finest
/// tile held over it gives it, is void.
fn void_under(at: &Pvf3) -> bool {
    crate::query::World::now().at(at, LEAF_ZOOM).value().is_some_and(|g| g.top().is_none())
}
