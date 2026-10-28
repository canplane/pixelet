// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, handed over
//! only when changed) and the commands it applies to the stores.

use crate::domain::layer::Base as _;
use pxl_spec::asset::cube::cube_id;
use pxl_spec::asset::decor_num_id;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::zoom::units;
use xpute_kit::wire::xtp::PacketWriter;

use crate::domain::control::camera::controller::{dist, Motion};
use crate::domain::control::camera::input::dragging;
use crate::domain::control::camera::state::camera_state;
use crate::domain::control::cmd::Tracking;
use crate::domain::control::cmd::{
    CompanionState, DivisionPart, FocusState, GroundStatus, HoldingState, LangName, LocationState,
    LonLat, Message, OrientationState, Snapshot,
};
use crate::domain::control::interaction::{holding, Holding};
use crate::domain::control::navigation::destination::go_to_ground;
use crate::domain::control::navigation::follow_mode::{following, set_following};
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::sensor::device_orientation::{orientation_close, request_orientation};
use crate::domain::control::sensor::geolocation::{
    geolocation_close, geolocation_open, start_watching,
};
use crate::domain::control::sensor::reading::{
    device_location_store, device_orientation_store, Pose,
};
use crate::domain::layer::division::{named, naming};
use crate::domain::scene::character::companion::companion_ready;
use crate::domain::scene::character::preview::{preview_rows, CASES_MAX};
use crate::domain::scene::stage::screen_drawn;
use crate::domain::view::frame::screen::units_per_px_at;
use crate::domain::view::setting::store::{settings_loaded, settings_store, SettingKey};
use crate::sys::io::file::{self, Kept};
use crate::turn::world_platform;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::tile::Tile;
use xpute_kit::status::errno::Errno;

/// The focus's division and each above it, broad to narrow. Every language
/// is sent, so switching language re-reads rather than refetches. A name
/// shows where a logical pixel, `u_per_px` wide, is no wider than a cell at
/// the zoom it shows from (`Division::shown_from`).
fn division_parts(at: Option<(Tile, u32)>, u_per_px: f64, each: &mut dyn FnMut(&DivisionPart<'_>)) {
    let Some(at) = at else { return };
    named(at, |part| {
        let names = |each: &mut dyn FnMut(&LangName<'_>)| {
            for (lang, name) in &part.names {
                each(&LangName { lang, name });
            }
        };
        each(&DivisionPart {
            subtype: part.subtype as u8,
            name: &part.name,
            names: &names,
            shown: u_per_px <= units(part.shown_from()),
            area: part.area,
        });
    });
}

/// What the snapshot is made of, read off its owners: where the names are
/// read (`division::naming`) rather than the names, and the waiting
/// marker's words aside (`Sent`).
#[derive(Clone, Copy, PartialEq)]
pub struct Shown {
    status: GroundStatus,
    is_off_center: bool,
    focus: Option<Q3>,
    location: (Pose, bool),
    companion: [bool; 2],
    holding: Holding,
    marker: bool,
    orientation: Pose,
    azimuth: f64,
    is_follow_mode: bool,
    camera_busy: bool,
    zooming: bool,
    over_marker: bool,
    /// By each setting's number, one not given out 0; none until they are
    /// read, as the shell holds none before its first snapshot.
    settings: Option<[u32; SettingKey::SLOTS]>,
    /// The view's scale at the focus, as the units a logical pixel spans.
    u_per_px: f64,
    division: Option<(Tile, u32)>,
    previews: ([u32; CASES_MAX], usize),
    stall: Option<Message>,
    declared: bool,
    /// The refused kept slots, the first `.1` of them.
    kept_refused: ([u32; Kept::ALL.len()], usize),
}

fn shown() -> Shown {
    let (location, settings) = (device_location_store(), settings_store());
    let focus = crate::domain::control::camera::controller::focus();
    let mut previews = ([0u32; CASES_MAX], 0);
    for (slot, (surface, _, _)) in previews.0.iter_mut().zip(preview_rows()) {
        *slot = surface;
        previews.1 += 1;
    }
    Shown {
        // Shown once a frame went to the screen over a view whose tiles all
        // show, if coarser: a finer tile asked for as the eye moves is not
        // the area loading, and would flash its toast each time.
        status: match (crate::domain::view::observation::eye(), screen_drawn()) {
            (None, _) => GroundStatus::Idle,
            (Some(_), true) if TerrainTile::residency().shown() => GroundStatus::Success,
            _ => GroundStatus::Loading,
        },
        is_off_center: off_center().is_off_center,
        focus,
        location: (location.pose, location.location.is_some()),
        companion: companion_ready(),
        holding: holding(),
        marker: crate::domain::control::interaction::edit::marker_waiting().0,
        orientation: device_orientation_store().pose,
        azimuth: camera_state().intent.azimuth,
        is_follow_mode: following(),
        camera_busy: dragging(),
        zooming: crate::domain::control::camera::controller::zooming(),
        over_marker: crate::pointer::over_marker(),
        settings: settings_loaded().then(|| {
            let mut by_number = [0; SettingKey::SLOTS];
            for key in SettingKey::ALL {
                by_number[key as usize] = settings.get(key);
            }
            by_number
        }),
        u_per_px: units_per_px_at(dist()),
        division: focus.and_then(|at| naming(&at)),
        previews,
        stall: stall(),
        declared: crate::sys::io::source::top().is_some(),
        kept_refused: {
            let mut refused = ([0; Kept::ALL.len()], 0);
            for kept in Kept::ALL.into_iter().filter(|&k| file::refused(k)) {
                refused.0[refused.1] = kept.slot();
                refused.1 += 1;
            }
            refused
        },
    }
}

/// The snapshot last sent, which a new one is sent only where it moved
/// from: its makings and the waiting marker's words.
#[derive(Default)]
pub struct Sent {
    shown: Option<Shown>,
    words: String,
}

impl Sent {
    /// What the snapshot is made of now, where it moved since it was last
    /// sent; taken as sent.
    pub fn moved(&mut self) -> Option<Shown> {
        let (now, (_, words)) = (
            shown(),
            crate::domain::control::interaction::edit::marker_waiting(),
        );
        if self.shown == Some(now) && self.words == words {
            return None;
        }
        self.shown = Some(now);
        self.words.clear();
        self.words.push_str(words);
        Some(now)
    }
}

/// The snapshot made of `shown` into `out`; none where it does not fit.
pub fn ui_snapshot(shown: &Shown, out: &mut [u8]) -> Option<u32> {
    let (_, words) = crate::domain::control::interaction::edit::marker_waiting();
    let held = shown.holding;
    let snapshot = Snapshot {
        focus: FocusState {
            status: shown.status,
            is_off_center: shown.is_off_center,
            center: shown.focus.map(|c| {
                let g = epsg4326::unproject(&c);
                LonLat {
                    lon: g[0],
                    lat: g[1],
                }
            }),
            outside: shown.focus.is_some_and(|c| {
                crate::sys::io::source::past_at([i64::from(c[0]), i64::from(c[1])].map(q3::to_real))
            }),
        },
        location: LocationState {
            status: shown.location.0.status(),
            has_location: shown.location.1,
            error: shown.location.0.error(),
        },
        companion: CompanionState {
            person_ready: shown.companion[0],
            pet_ready: shown.companion[1],
        },
        holding: HoldingState {
            tool: held.active_tool,
            cube: cube_id(held.object_cube),
            decor: decor_num_id(held.decor_id),
            tier: held.object_tier,
            marker: shown.marker,
            words: &words,
        },
        orientation: OrientationState {
            status: shown.orientation.status(),
            error: shown.orientation.error(),
        },
        azimuth: shown.azimuth,
        is_follow_mode: shown.is_follow_mode,
        camera_busy: shown.camera_busy,
        zooming: shown.zooming,
        over_marker: shown.over_marker,
        settings: shown.settings.as_ref().map_or(&[], |s| s),
        m_per_px: m_per_px(shown.u_per_px, shown.focus),
        division: &|each| division_parts(shown.division, shown.u_per_px, each),
        previews: &shown.previews.0[..shown.previews.1],
        kept_refused: &shown.kept_refused.0[..shown.kept_refused.1],
        stall: shown.stall,
    };
    let mut w = PacketWriter::new(out, Snapshot::FIELDS);
    snapshot.write(&mut w);
    w.fitted()
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
            crate::domain::control::camera::controller::reset_azimuth();
        }
    }
}

/// A search's place: a view far out comes in.
pub fn go_to_degrees(lon: f64, lat: f64) {
    crate::domain::control::navigation::destination::go_on_ground(
        epsg4326::project(&[lon, lat, 0.0]),
        Motion::Snap,
    );
    crate::domain::control::camera::controller::come_in_to_default();
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
    if crate::sys::io::request::given_up() {
        return Some(Message::WorldUnavailable);
    }
    match crate::sys::io::request::failing() {
        Some(Errno::ENETUNREACH) | None => None,
        Some(_) => Some(Message::WorldReadFailing),
    }
}

/// The view's scale at the focus, `u_per_px`, as the meters a logical pixel
/// spans; 0 where the camera stands at the center: no distance, no scale.
fn m_per_px(u_per_px: f64, focus: Option<Q3>) -> f64 {
    let y_u = focus.map_or(0.0, |c| q3::to_real(i64::from(c[1])));
    u_per_px / epsg3857::u_per_m(y_u)
}
