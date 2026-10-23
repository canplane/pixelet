// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

use crate::layer::Base as _;
use pxl_spec::block::object::{cube_id, decor_num_id};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::q3::{self, Q3};
use xpute_kit::wire::xtp::PacketWriter;

use crate::abi::cmd::Tracking;
use crate::abi::cmd::{CompanionState, DivisionPart, FocusState, GroundStatus, HoldingState, LangName, LocationState, LonLat, Message, OrientationState, Snapshot, SourceVersion};
use crate::control::camera::controller::{dist, focus, Motion};
use crate::control::camera::input::dragging;
use crate::control::camera::state::camera_state;
use crate::control::interaction::holding;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::follow_mode::{following, set_following};
use crate::control::navigation::off_center::off_center;
use crate::control::sensor::device_orientation::{orientation_close, request_orientation};
use crate::control::sensor::geolocation::{geolocation_close, geolocation_open, start_watching};
use crate::control::sensor::reading::{device_location_store, device_orientation_store};
use crate::layer::division::named;
use crate::scene::character::companion::companion_ready;
use crate::scene::character::preview::{preview_rows, CASES_MAX};
use crate::scene::stage::screen_drawn;
use crate::setting::store::{settings_store, SettingKey};
use crate::turn::world_platform;
use crate::view::frame::pose::units_per_px_at;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_kit::status::errno::Errno;

/// The focus's division and each above it, broad to narrow. Every language
/// is sent, so switching language re-reads rather than refetches.
fn division_parts(at: Option<Q3>, each: &mut dyn FnMut(&DivisionPart<'_>)) {
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
            area: part.area,
        });
    });
}

/// None where it does not fit.
pub fn ui_snapshot(out: &mut [u8]) -> Option<u32> {
    let location = device_location_store();
    let orientation = device_orientation_store();
    let camera = camera_state();
    let settings = settings_store();
    let [person_ready, pet_ready] = companion_ready();
    let at = crate::control::camera::controller::focus();
    let center = at.map(|c| {
        let g = epsg4326::unproject(&c);
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
            // Shown once a frame went to the screen over a view whose tiles all answered.
            status: match (crate::view::observation::eye(), screen_drawn()) {
                (None, _) => GroundStatus::Idle,
                (Some(_), true) if TerrainTile::residency().answered() => GroundStatus::Success,
                _ => GroundStatus::Loading,
            },
            is_off_center: off_center().is_off_center,
            center,
            outside: at.is_some_and(|c| crate::io::source::past_at([i64::from(c[0]), i64::from(c[1])].map(q3::to_real))),
        },
        location: LocationState {
            status: location.pose.status(),
            has_location: location.location.is_some(),
            error: location.pose.error(),
        },
        companion: CompanionState { person_ready, pet_ready },
        holding: HoldingState {
            tool: held.active_tool,
            cube: cube_id(held.object_cube),
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
        camera_busy: dragging(),
        zooming: crate::control::camera::controller::zooming(),
        over_badge: crate::door::over_badge(),
        settings: &values,
        m_per_px: m_per_px(),
        division: &|each| division_parts(at, each),
        previews: &previews[..preview_cnt],
        stall: stall(),
        source: &|each| {
            crate::io::source::versions(&mut |kind, version| {
                each(&SourceVersion {
                    kind: &kind.name(),
                    version: &version,
                })
            })
        },
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
            // On the floor nearest the fix's altitude, or the surface where it has
            // none; the distance is kept.
            if let Some(device) = off_center().device_coord {
                go_to_ground(device, Motion::Snap);
            }
            set_following(false);
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

/// A search's place: a view far out comes in.
pub fn go_to_degrees(lon: f64, lat: f64) {
    crate::control::navigation::destination::go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
    crate::control::camera::controller::come_in_to_default();
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

/// Why the server does not answer the world's reads now: failing, or given up.
/// The network and the GPU are the host's, which says them.
fn stall() -> Option<Message> {
    if crate::io::request::given_up() {
        return Some(Message::WorldUnavailable);
    }
    match crate::io::request::failing() {
        Some(Errno::ENETUNREACH) | None => None,
        Some(_) => Some(Message::WorldReadFailing),
    }
}

/// The view's scale at the focus, as the meters a logical pixel spans;
/// 0 where the camera stands at the center: no distance, no scale.
fn m_per_px() -> f64 {
    let y_u = focus().map_or(0.0, |c| q3::to_real(i64::from(c[1])));
    units_per_px_at(dist()) / epsg3857::u_per_m(y_u)
}
