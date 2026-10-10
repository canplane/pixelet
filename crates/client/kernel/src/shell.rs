// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

use crate::view::demand::Observed as _;
use pxl_spec::block::object::{cube_id, decor_num_id};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use xpute_kit::wire::xtp::PacketWriter;

use crate::abi::cmd::{CompanionState, DivisionPart, FocusState, GroundStatus, HoldingState, LangName, LocationState, LonLat, Message, Snapshot};
use crate::control::camera::controller::{dist, focus, Motion};
use crate::control::camera::state::{camera_state, CameraStatus, IntentPatch};
use crate::control::interaction::holding;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::off_center::off_center;
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::reading::device_location_store;
use crate::layer::division::named;
use crate::scene::character::companion::companion_load_store;
use crate::scene::character::preview::{preview_rows, CASES_MAX};
use crate::scene::stage::{screen, Screen};
use crate::setting::store::{settings_store, SettingKey};
use crate::turn::world_platform;
use crate::view::frame::pose::units_per_px_at;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::status::errno::Errno;

/// The focus's division and each above it, broad to narrow. Every language
/// is sent, so switching language re-reads rather than refetches.
fn division_parts(at: Option<Pvf3>, each: &mut dyn FnMut(&DivisionPart<'_>)) {
    let Some(at) = at else { return };
    named(&at, |part| {
        let names = |each: &mut dyn FnMut(&LangName<'_>)| {
            for (lang, name) in &part.names {
                each(&LangName { lang, name });
            }
        };
        each(&DivisionPart {
            subtype: part.subtype as u8,
            name: &part.name,
            names: &names,
            prominence: part.prominence,
        });
    });
}

/// None where it does not fit.
pub fn ui_snapshot(out: &mut [u8]) -> Option<u32> {
    let location = device_location_store();
    let camera = camera_state();
    let settings = settings_store();
    let companion = companion_load_store();
    let at = crate::control::camera::controller::focus();
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
            // Shown once a frame that drew ground went to the screen, or one went over
            // a view whose tiles all answered, which over a source's void draws nothing.
            status: match (crate::view::observation::eye(), screen()) {
                (None, _) => GroundStatus::Idle,
                (Some(_), Screen::Drawn { ground: true }) => GroundStatus::Success,
                (Some(_), Screen::Drawn { ground: false }) if TerrainTile::demand().answered() => GroundStatus::Success,
                _ => GroundStatus::Loading,
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
            cube: cube_id(held.object_cube),
            decor: decor_num_id(held.decor_id),
            tier: held.object_tier,
            marker: waiting,
            words: &words,
        },
        azimuth: camera.intent.azimuth,
        camera_busy: camera.camera_status != CameraStatus::Idle,
        zooming: crate::control::camera::controller::zooming(),
        settings: &values,
        m_per_px: m_per_px(),
        division: &|each| division_parts(at, each),
        previews: &previews[..preview_cnt],
        stall: stall(),
    };
    let mut w = PacketWriter::new(out, Snapshot::FIELDS);
    snapshot.write(&mut w);
    w.finish()
}

/// The location button's press: the view moves to the device, its distance
/// and heading kept.
pub fn recenter() {
    // A fix's Z is altitude, absent on most and never the ground's height.
    if let Some(device) = off_center().device_coord {
        go_to_ground(device, Motion::Snap);
    }
}

/// A search's place: a view far out comes in.
pub fn go_to_degrees(lon: f64, lat: f64) {
    crate::control::navigation::destination::go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
    crate::control::camera::controller::come_in_to_default();
}

/// Set outright, for kernel tests and A/B comparisons at the same view.
pub fn set_view(dist: Option<f64>, polar: Option<f64>) {
    crate::control::camera::controller::set_intent(IntentPatch { dist, azimuth: None, polar });
}

pub fn location_enable() {
    start_watching(world_platform().geolocation);
}

pub fn sensors_open() {
    geolocation_open(world_platform().geolocation);
}

pub fn sensors_close() {
    geolocation_close();
}

/// Why the world is not coming now: the device gone, or reads failing.
fn stall() -> Option<Message> {
    match (screen(), crate::io::request::failing()) {
        (Screen::Lost, _) => Some(Message::WorldGpuLost),
        (_, Some(Errno::ENETUNREACH)) => Some(Message::WorldOffline),
        (_, Some(_)) => Some(Message::WorldReadFailing),
        (_, None) => None,
    }
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
    crate::query::World::now().surface(at, LEAF_ZOOM).value().is_some_and(|s| s.is_none())
}
