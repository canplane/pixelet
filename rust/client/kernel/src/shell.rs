// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel (a Snapshot packet, and the view's
//! azimuth and scale beside it, each handed over only when changed) and the
//! commands it applies to the stores.

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
use crate::domain::scene::character::preview::{preview_rows, PREVIEWS_MAX};
use crate::domain::scene::stage::is_screen_drawn;
use crate::domain::view::frame::pose;
use crate::domain::view::frame::screen::units_per_px_at;
use crate::domain::view::setting::store::{are_settings_loaded, settings_store, SettingKey};
use crate::door::cmd::{
    CompanionState, DivisionPart, GroundStatus, LocationState, LonLat, Message, OrientationState,
    PivotState, RefusedFile, ShellView, Snapshot,
};
use crate::pointer;
use crate::sys::io::file::{self, Path, StorageFile};
use crate::sys::io::file_spec::root;
use crate::sys::io::request;
use crate::sys::io::source;
use pxl_render::gpu::renderer::renderer_progress;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::errno::Errno;

/// The finest zoom whose side a logical pixel, `u_per_px` wide, is no wider
/// than; none where it is wider than the world. A division's name shows where
/// the zoom it shows from (`Division::shown_from`) is no finer than this one,
/// so the names shown move only where the view's scale crosses a zoom.
fn named_to(u_per_px: f64) -> Option<Zoom> {
    (0..=FINEST_ZOOM).rev().find(|&z| u_per_px <= units(z))
}

/// The pivot's division and each above it, broad to narrow, each shown where
/// the view names to its zoom (`named_to`).
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

/// What the snapshot is made of, read off its owners: where the names are
/// read (`division::naming`) rather than the names, which of them show rather
/// than the view's scale, and the waiting marker's text aside (`SnapshotMemo`).
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
    /// By each setting's number, one not given out 0; none until they are
    /// read, as the shell holds none before its first snapshot.
    settings: Option<[u32; SettingKey::SLOTS]>,
    division: Option<(Quadrant, u32)>,
    /// The zoom the view names its divisions to (`named_to`).
    named_to: Option<Zoom>,
    previews: ([u32; PREVIEWS_MAX], usize),
    stall: Option<Message>,
    declared: bool,
    /// The refused files by root and path, the first `.1` of them.
    refused_files: ([(u32, Path); REFUSED_SHOWN], usize),
    renderer_progress: f64,
    area_progress: f64,
}

/// The refused files a snapshot names at once: every storage file, and as
/// many of the objects' as the view asks for at once; the rest are named as
/// the client empties those.
const REFUSED_SHOWN: usize = StorageFile::ALL.len() + repository::WANTS as usize;

fn snapshot_inputs() -> SnapshotInputs {
    let (location, settings) = (device_location_store(), settings_store());
    let pivot = controller::pivot();
    let division = pivot.and_then(|at| naming(&at));
    let mut previews = ([0u32; PREVIEWS_MAX], 0);
    for (slot, (surface, ..)) in previews.0.iter_mut().zip(preview_rows()) {
        *slot = surface;
        previews.1 += 1;
    }
    // Shown once a frame went to the screen over a view whose tiles all
    // show, if coarser: a finer tile asked for as the eye moves is not
    // the area loading, and would flash its toast each time.
    let status = match (pose::eye(), is_screen_drawn()) {
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
        previews,
        stall: stall(),
        declared: source::top().is_some(),
        refused_files: {
            let stored = StorageFile::ALL
                .into_iter()
                .filter(|&k| file::is_refused(k))
                .map(|k| (root::STORAGE, k.path()));
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
        // Asked of every tile, so only while the area loads.
        area_progress: match status {
            GroundStatus::Success => 1.0,
            _ => TerrainTile::tile_cache().shown_share(),
        },
    }
}

/// The snapshot last sent, which a new one is sent only where it moved
/// from: its makings and the marker draft's text; and the view last sent,
/// its azimuth and its meters a pixel.
#[derive(Default)]
pub struct SnapshotMemo {
    last_inputs: Option<SnapshotInputs>,
    text: String,
    last_view: Option<(f64, f64)>,
}

impl SnapshotMemo {
    /// What the snapshot is made of now, where it moved since it was last
    /// sent; taken as sent.
    pub fn take_moved(&mut self) -> Option<SnapshotInputs> {
        let (now, text) = (snapshot_inputs(), write::marker_draft().unwrap_or_default());
        if self.last_inputs == Some(now) && self.text == text {
            return None;
        }
        self.last_inputs = Some(now);
        self.text.clear();
        self.text.push_str(text);
        Some(now)
    }

    /// The view now, where its azimuth or its scale moved since it was last
    /// sent; taken as sent.
    pub fn take_view_moved(&mut self) -> Option<ShellView> {
        let now = (
            camera_state().intent.azimuth,
            m_per_px(units_per_px_at(dist()), controller::pivot()),
        );
        if self.last_view == Some(now) {
            return None;
        }
        self.last_view = Some(now);
        Some(ShellView {
            azimuth: now.0,
            m_per_px: now.1,
        })
    }
}

/// The snapshot made of `shown` into `out`; none where it does not fit.
pub fn ui_snapshot(shown: &SnapshotInputs, out: &mut [u8]) -> Option<u32> {
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
        previews: &shown.previews.0[..shown.previews.1],
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

/// Why the server does not answer the world's reads now: failing, or given up.
/// The network and the GPU are the client's, which says them.
fn stall() -> Option<Message> {
    if request::given_up() {
        return Some(Message::WorldUnavailable);
    }
    match request::failing() {
        Some(Errno::ENETUNREACH) | None => None,
        Some(_) => Some(Message::WorldReadFailing),
    }
}

/// The view's scale at the pivot, `u_per_px`, as the meters a logical pixel
/// spans; 0 where the camera stands at the pivot: no distance, no scale.
fn m_per_px(u_per_px: f64, pivot: Option<Q3>) -> f64 {
    let y_u = pivot.map_or(0.0, |c| q3::to_real(i64::from(c[1])));
    u_per_px / epsg3857::u_per_m(y_u)
}
