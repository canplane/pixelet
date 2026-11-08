// pxl-core/ui.rs

//! What the UI reads of the core: a Snapshot packet, and beside it the view's
//! azimuth and scale. Each is handed over only when it has changed since it
//! was last sent.

use crate::domain::layer::Base as _;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::zoom::{units, Zoom, FINEST_ZOOM};
use xpute_kit::wire::xtp::PacketWriter;

use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::dist;
use crate::domain::control::camera::state::camera_state;
use crate::domain::control::edit::write;
use crate::domain::control::input::{dragging, zooming};
use crate::domain::control::navigation::follow_mode::following;
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::sensor::reading::{
    device_location_store, device_orient_store, SensorState,
};
use crate::domain::layer::division::{each_division, naming};
use crate::domain::layer::object::repository;
use crate::domain::query::World;
use crate::domain::scene::character::companion::companion_ready;
use crate::domain::scene::character::surface::character_surface_rows;
use crate::domain::scene::stage::screen_shows_world;
use crate::domain::setting::store::{are_settings_loaded, settings_store, SettingKey};
use crate::domain::view::frame::pose;
use crate::domain::view::frame::screen::units_per_px_at;
use crate::door::cmd::{
    CompanionState, DivisionPart, GroundStatus, LocationState, LonLat, Message, OrientationState,
    PivotState, RefusedFile, Snapshot, UiView,
};
use crate::pointer;
use crate::sys::io::file::{self, Path};
use crate::sys::io::file_spec::root;
use crate::sys::io::request;
use crate::sys::io::source;
use pxl_render::gpu::renderer::renderer_progress;
use pxl_render::gpu::surface::SURFACES;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::errno::Errno;

/// The finest zoom whose tile side is at least one logical pixel, where a
/// pixel spans `u_per_px` units. It is None where a pixel spans more than the
/// world. A division's name shows when the zoom it shows from
/// (`Division::shown_from`) is no finer than this zoom, so the shown names
/// change only when the view's scale crosses a zoom.
fn named_to(u_per_px: f64) -> Option<Zoom> {
    (0..=FINEST_ZOOM).rev().find(|&z| u_per_px <= units(z))
}

/// The pivot's division and every division above it, from the broadest to
/// the narrowest. A division is shown when the view names down to its zoom
/// (`named_to`).
fn division_parts(
    at: Option<(Quadrant, u32)>,
    named_to: Option<Zoom>,
    each: &mut dyn FnMut(&DivisionPart<'_>),
) {
    let Some(at) = at else { return };
    each_division(at, |part| {
        each(&DivisionPart {
            subtype: part.subtype as u8,
            name: &part.name,
            shown: named_to.is_some_and(|z| part.shown_from() <= z),
            area: part.area,
        });
    });
}

/// The values a snapshot is built from, read from their owners and compared
/// to decide whether a new snapshot is due. They hold what changes rarely:
/// the division the names are read from (`division::naming`) rather than the
/// names, and the zoom the names show down to rather than the view's scale.
/// The marker draft's text is kept apart, in `SnapshotMemo`.
#[derive(Clone, Copy, PartialEq)]
pub struct SnapshotInputs {
    status: GroundStatus,
    is_off_center: bool,
    pivot: Option<Q3>,
    location: (SensorState, bool),
    companion: [bool; 2],
    drafting: bool,
    orient: SensorState,
    is_follow_mode: bool,
    camera_busy: bool,
    zooming: bool,
    over_marker: bool,
    /// Each setting's value at its number, and 0 at a number no setting has.
    /// It is None until the settings are read, and the UI holds no settings
    /// before its first snapshot.
    settings: Option<[u32; SettingKey::SLOTS]>,
    division: Option<(Quadrant, u32)>,
    /// The zoom the view names its divisions to (`named_to`).
    named_to: Option<Zoom>,
    character_surfaces: ([u32; SURFACES], usize),
    stall: Option<Message>,
    declared: bool,
    /// The refused files as root and path. Only the first `.1` are filled.
    refused_files: ([(u32, Path); REFUSED_SHOWN], usize),
    renderer_progress: f64,
    area_progress: f64,
}

/// How many refused files one snapshot names: the storage file, and as many
/// object files as the view asks for at once. The rest are named in later
/// snapshots, as the client empties the ones already named.
const REFUSED_SHOWN: usize = 1 + repository::WANTS as usize;

fn snapshot_inputs() -> SnapshotInputs {
    let (location, settings) = (device_location_store(), settings_store());
    let pivot = controller::pivot();
    let division = pivot.and_then(|at| naming(&at));
    let mut character_surfaces = ([0u32; SURFACES], 0);
    for (slot, (surface, ..)) in character_surfaces
        .0
        .iter_mut()
        .zip(character_surface_rows())
    {
        *slot = surface;
        character_surfaces.1 += 1;
    }
    // The ground counts as loaded once a frame has reached the screen over a
    // view whose tiles all show, even where some show at a coarser zoom. A
    // finer tile asked for as the eye moves is not the area loading, and
    // counting it would flash the loading toast each time.
    let status = match (pose::eye(), screen_shows_world()) {
        (None, _) => GroundStatus::Idle,
        (Some(_), true) if TerrainTile::tile_cache().is_shown() => GroundStatus::Success,
        _ => GroundStatus::Loading,
    };
    SnapshotInputs {
        status,
        is_off_center: off_center().is_off_center,
        pivot,
        location: (location.state, location.location.is_some()),
        companion: companion_ready(),
        drafting: write::marker_draft().is_some(),
        orient: device_orient_store().state,
        is_follow_mode: following(),
        camera_busy: dragging(),
        zooming: zooming(),
        over_marker: pointer::over_marker(),
        settings: are_settings_loaded().then(|| {
            let mut by_number = [0; SettingKey::SLOTS];
            for key in SettingKey::ALL {
                by_number[key as usize] = settings.get(key);
            }
            by_number
        }),
        division,
        named_to: named_to(units_per_px_at(dist())),
        character_surfaces,
        stall: stall(),
        declared: source::top().is_some(),
        refused_files: {
            let stored = file::is_storage_refused()
                .then(|| (root::STORAGE, file::storage_path()))
                .into_iter();
            let objects = repository::refused_files()
                .iter()
                .map(|&s| (root::OBJECT, Path::of_quadrant(s)));
            let mut refused = ([(0, Path::EMPTY); REFUSED_SHOWN], 0);
            for (at, file) in refused.0.iter_mut().zip(stored.chain(objects)) {
                *at = file;
                refused.1 += 1;
            }
            refused
        },
        renderer_progress: renderer_progress(),
        // The share is read from every tile, so it is computed only while
        // the area loads.
        area_progress: match status {
            GroundStatus::Success => 1.0,
            _ => TerrainTile::tile_cache().shown_share(),
        },
    }
}

/// What was last sent to the UI: the snapshot's inputs with the marker
/// draft's text, and the view's azimuth and meters per pixel. A new snapshot
/// or view is sent only when it differs from these.
#[derive(Default)]
pub struct SnapshotMemo {
    last_inputs: Option<SnapshotInputs>,
    text: String,
    last_view: Option<(f64, f64)>,
}

impl SnapshotMemo {
    /// The snapshot's inputs now, or None when they have not changed since
    /// they were last sent. What it returns counts as sent.
    pub fn take_changed(&mut self) -> Option<SnapshotInputs> {
        let (now, text) = (snapshot_inputs(), write::marker_draft().unwrap_or_default());
        if self.last_inputs == Some(now) && self.text == text {
            return None;
        }
        self.last_inputs = Some(now);
        self.text.clear();
        self.text.push_str(text);
        Some(now)
    }

    /// The view now, or None when neither its azimuth nor its scale has
    /// changed since it was last sent. What it returns counts as sent.
    pub fn take_view_changed(&mut self) -> Option<UiView> {
        let now = (
            camera_state().intent.azimuth,
            m_per_px(units_per_px_at(dist()), controller::pivot()),
        );
        if self.last_view == Some(now) {
            return None;
        }
        self.last_view = Some(now);
        Some(UiView {
            azimuth: now.0,
            m_per_px: now.1,
        })
    }
}

/// Writes the snapshot built from `shown` into `out`. Returns None when it
/// does not fit.
pub fn snapshot(shown: &SnapshotInputs, out: &mut [u8]) -> Option<u32> {
    let snapshot = Snapshot {
        pivot: PivotState {
            status: shown.status,
            is_off_center: shown.is_off_center,
            lon_lat: shown.pivot.map(|c| {
                let g = epsg4326::unproject(&c);
                LonLat {
                    lon: g[0],
                    lat: g[1],
                }
            }),
            outside: shown.pivot.is_some_and(|c| World::now().past(&c)),
        },
        location: LocationState {
            status: shown.location.0.status,
            has_location: shown.location.1,
            err: shown.location.0.err,
        },
        companion: CompanionState {
            person_ready: shown.companion[0],
            pet_ready: shown.companion[1],
        },
        draft: write::marker_draft(),
        orient: OrientationState {
            status: shown.orient.status,
            err: shown.orient.err,
        },
        is_follow_mode: shown.is_follow_mode,
        camera_busy: shown.camera_busy,
        zooming: shown.zooming,
        over_marker: shown.over_marker,
        settings: shown.settings.as_ref().map_or(&[], |s| s),
        division: &|each| division_parts(shown.division, shown.named_to, each),
        character_surfaces: &shown.character_surfaces.0[..shown.character_surfaces.1],
        refused_files: &|each| {
            for (root, path) in &shown.refused_files.0[..shown.refused_files.1] {
                each(&RefusedFile { root: *root, path });
            }
        },
        stall: shown.stall,
        renderer_progress: shown.renderer_progress,
        area_progress: shown.area_progress,
    };
    let mut w = PacketWriter::new(out, Snapshot::FIELDS);
    snapshot.write(&mut w);
    w.fit_len()
}

/// Why the server is not answering the world's reads now: the reads are
/// failing, or the core has given up on them. An unreachable network is left
/// out, because the client reports the network and the GPU itself.
fn stall() -> Option<Message> {
    if request::given_up() {
        return Some(Message::WorldUnavailable);
    }
    match request::failing() {
        Some(Errno::ENETUNREACH) | None => None,
        Some(_) => Some(Message::WorldReadFailing),
    }
}

/// The view's scale at the pivot, given as `u_per_px` units per logical
/// pixel, in meters per logical pixel. The Mercator scale depends on the
/// pivot's latitude. It is 0 when the camera stands at the pivot, where there
/// is no distance and so no scale.
fn m_per_px(u_per_px: f64, pivot: Option<Q3>) -> f64 {
    let y_u = pivot.map_or(0.0, |c| q3::to_real(i64::from(c[1])));
    u_per_px / epsg3857::u_per_m(y_u)
}
