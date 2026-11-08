// pxl-core/domain/scene/decor/gpu.rs

//! What the core hands the GPU's decor each frame (pxl-render
//! gpu/generator.rs). The GPU generates the decor the ground scatters and
//! draws it together with the decor users placed. The core hands it only what
//! the CPU alone knows, and never reads back what the GPU generated. Among
//! what it hands are the boxes where objects were written since the last
//! frame, because the ground's generated decor may have changed there.

use pxl_content::generation::{self, drawn, row_of};
use pxl_content::scatter;
use pxl_engine::object;
use pxl_engine::LEAF_CELL_ZOOM;
use pxl_render::gpu::api;
use pxl_render::gpu::draw::Position;
use pxl_render::gpu::generator::DecorFrame;
use pxl_spec::asset::decor::DECORS;
use pxl_spec::space::camera::grid::{split_dist, CLIP_CELLS_A_RAD, CLIP_MARGIN};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{idx_of, units, Zoom, UNIT_ZOOM};
use xpute_core::global::Global;

use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::preview::RenderEdits;
use crate::domain::query::World;
use crate::domain::scene::decor::param;
use crate::domain::setting::store::settings_store;
use crate::sys::io::loader::catalog;
use pxl_content::decor;
use pxl_engine::clipmap::FIRST_CELL_ZOOM;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::camera::grid;
use pxl_spec::space::layer::lattice;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant;

/// The angle about the eye, in radians, below which a decor is drawn as an
/// impostor instead of a model: `DECOR_MODEL_CELLS` camera grid cells
/// (pxl-spec space/camera/grid.rs `CELL`). Being an angle, it does not change
/// with the screen's size or aspect ratio.
pub fn decor_model_angle() -> f64 {
    param::DECOR_MODEL_CELLS * grid::CELL
}

/// The angle about the eye, in radians, below which a decor's impostor is
/// culled: `DECOR_IMPOSTOR_CELLS` camera grid cells.
pub fn decor_impostor_angle() -> f64 {
    param::DECOR_IMPOSTOR_CELLS * grid::CELL
}

/// The distance, in units, past which a clip level culls a decor `extent_u`
/// wide: where it spans `DECOR_IMPOSTOR_CELLS` of the clip level's cells
/// (`CLIP_CELLS_A_RAD`). A clip level's cells are `CLIP_MARGIN` times finer
/// than the view's, so this distance is `CLIP_MARGIN` times the one at which
/// the view culls it (`decor_impostor_angle`), an upper bound.
/// `is_decor_shown` scales its other side by the same margin, so the zooms it
/// picks are the ones the view would pick.
pub const fn cull_dist_u(extent_u: f64) -> f64 {
    extent_u * CLIP_CELLS_A_RAD / param::DECOR_IMPOSTOR_CELLS
}

/// Whether a decor `extent_u` wide is still large enough for an impostor
/// somewhere cells of `cell_zoom` are drawn. The nearest such place is where
/// that zoom begins, so the decor is measured there.
fn is_decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    split_dist(cell_zoom) * CLIP_MARGIN <= cull_dist_u(extent_u)
}

/// The clip level zooms each kind is generated at, one bit for each zoom from
/// `FIRST_CELL_ZOOM`. A kind is generated at the leaves' zoom, and at each
/// coarser zoom where one of its variants stands as itself, by its own size
/// rather than the clip level's cell (pxl-content `scatter::stands_at`), and
/// is still large enough for an impostor. Every one of those zooms reads the
/// decor of the same cells. No zoom finer than the leaves' is needed, since a
/// finer clip level lies within the leaves' one.
fn shown() -> [u32; DECORS.len()] {
    shown_by(|d| d.extent(catalog()))
}

/// `shown`, with each kind's extent taken from `extent` instead of the
/// catalog.
fn shown_by(extent: impl Fn(&Decor) -> f64) -> [u32; DECORS.len()] {
    core::array::from_fn(|k| {
        let (d, extent) = (&DECORS[k], extent(&DECORS[k]));
        (FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM)
            .filter(|&cell_zoom| {
                let stands = scatter::any_stands_at(d.row, cell_zoom);
                stands && is_decor_shown(extent, cell_zoom)
            })
            .fold(0, |bits, cell_zoom| {
                bits | 1 << (cell_zoom - FIRST_CELL_ZOOM)
            })
    })
}

/// What this module hands the GPU each frame. It is kept between frames, so a
/// frame where nothing changed allocates nothing, and each part is rebuilt
/// only when its inputs change.
struct Frame {
    shown: [u32; DECORS.len()],
    walk_zoom: Zoom,
    drawing: [u32; drawn::DRAWING_WORDS as usize],
    placed: Vec<[u32; generation::RECORD_WORDS as usize]>,
    /// The column boxes, in leaf cells, where objects were written since a
    /// frame last took them (`note_edited`). The first `edited` of them are
    /// the ones the next frame hands.
    edits: [[i32; 4]; object::EDITS as usize],
    edited: usize,
    /// The unit cell the eye stands in, as a draw's Q3 position (pxl-render
    /// gpu/draw.rs `Position`). The GPU draws the decor's lanes about it.
    anchor: Q3,
    /// The inputs `drawing`, `shown` and `walk_zoom` were last built from.
    seen: Option<Key>,
    /// What the last walk of placed decor was made over.
    walked: Option<WalkKey>,
}

/// The inputs the drawing words are built from, compared by value. The
/// catalog measures the variants, so whether it has been read is one of them.
/// An input the drawing words read must be added here, or they are never
/// rebuilt when it changes. A motion of the eye within its unit cell changes
/// nothing here, because the GPU reads it from the frame's eye (pxl-render
/// word.rs `frame::EYE`).
#[derive(Clone, Copy, PartialEq)]
struct Key {
    anchor: V3,
    scattered_cell: Option<[i32; 3]>,
    read: bool,
}

/// What a walk of placed decor was made over: the eye's cell at the walk's
/// zoom (`walk_zoom`) and the placed decor the eraser's preview hides. The
/// placed decor is walked again when this changes or when a write meets the
/// walked box (`note_edited`), so one walk serves while the eye stays in its
/// cell.
#[derive(Clone, Copy, PartialEq)]
struct WalkKey {
    zoom: Zoom,
    cell: [i32; 3],
    hide: Option<Octant>,
}

static HAND_OVER: Global<Frame> = Global::new();

fn hand_over() -> &'static mut Frame {
    HAND_OVER.get(|| Frame {
        shown: [0; DECORS.len()],
        walk_zoom: UNIT_ZOOM,
        drawing: [0; drawn::DRAWING_WORDS as usize],
        placed: Vec::new(),
        edits: [[0; 4]; object::EDITS as usize],
        edited: 0,
        anchor: [0; 3],
        seen: None,
        walked: None,
    })
}

/// A leaf cell's X wrapped as the world wraps it (pxl-engine `wrap_idx`), as a
/// word. The GPU's generated records name X the same way.
fn wrap(x: i32) -> u32 {
    pxl_engine::wrap_idx(LEAF_CELL_ZOOM, x) as u32
}

/// Builds this frame's hand-over for an eye at `eye`. Placed decor is walked
/// out to the reach `walk_zoom` describes. The placed decor and the ground's
/// decor that the eraser's preview hides (`edits`) are left out.
pub fn step(eye: V3, edits: &RenderEdits) {
    let RenderEdits {
        hide,
        scattered_cell,
        ..
    } = *edits;
    let h = hand_over();
    let anchor = eye.map(libm::floor);
    h.anchor = Position::of(anchor).q;
    let seen = Key {
        anchor,
        scattered_cell,
        read: catalog().is_some(),
    };
    // Only a hidden decor is left out of the records. The rays cut any other
    // hidden object, so hiding one keeps the walk.
    let hide = hide.filter(|&at| {
        World::now()
            .entry_at(at)
            .is_some_and(|e| e.asset().decor().is_some())
    });
    if h.seen != Some(seen) {
        h.seen = Some(seen);
        drawing(&mut h.drawing, seen);
        h.shown = shown();
        h.walk_zoom = walk_zoom();
    }
    let zoom = h.walk_zoom;
    let side = units(zoom);
    let cell = anchor.map(|v| libm::floor(v / side) as i32);
    let around = Aabb {
        min: [0, 1, 2].map(|k| (cell[k] - 1) as f64 * side),
        max: [0, 1, 2].map(|k| (cell[k] + 2) as f64 * side),
    };
    let walked = WalkKey { zoom, cell, hide };
    let met = h.edits[..h.edited].iter().any(|e| meets(e, &around));
    if h.walked == Some(walked) && !met {
        return;
    }
    // The renderer hands the records to the runtime in place, without a copy.
    // While the runtime still holds them they must not change, so the walk
    // waits for a later frame (pxl-render gpu/api.rs `in_flight`).
    if api::in_flight(&h.placed) {
        h.walked = None;
        return;
    }
    h.walked = Some(walked);
    h.placed.clear();
    let placed = &mut h.placed;
    World::now().placed_in(&around, &mut |e| {
        let (k, decor) = row_of(&decor::DECORS, e.state);
        if !decor || hide == Some(e.id()) {
            return;
        }
        let (variant, turn) = user_decor_pick(&e.corner(), &DECORS[k as usize]);
        let [x, y, z] = idx_of(UNIT_ZOOM, &e.corner());
        placed.push([
            generation::record_word(
                k,
                variant,
                turn,
                scatter::coarsest_stand(&decor::DECORS[k as usize], variant),
            ),
            wrap(x),
            y as u32,
            z as u32,
            0,
        ]);
    });
}

/// The zoom at which placed decor is walked: that of the finest cell at least
/// as wide as the reach. The reach ends where the largest decor is too small
/// for an impostor, or where the rays stop asking the leaves' cells, since no
/// decor is drawn past that. With cells this wide, the eye's cell and the
/// cells beside it hold the whole reach from any point in the eye's cell. A
/// walked decor beyond the reach of the eye's actual position is culled by
/// size on the GPU (`decor_cnt_cs`), as anything outside the walked box would
/// be.
fn walk_zoom() -> Zoom {
    let dist = (DECORS
        .iter()
        .map(|d| d.extent(catalog()))
        .fold(0.0, f64::max)
        / decor_impostor_angle())
    .min(lattice::draw_dist(lattice::GROUND, quadrant::LEAF_ZOOM));
    let mut zoom = UNIT_ZOOM;
    while zoom > 0 && units(zoom) < dist {
        zoom -= 1;
    }
    zoom
}

/// Whether an edit's columns, `[x0, y0, x1, y1)` in leaf cells, meet those of
/// `around`. X goes round the world, and the edit's X is on whatever lap it
/// was written on, so the test tries it on every lap that could meet.
fn meets(e: &[i32; 4], around: &Aabb) -> bool {
    let leaf = units(LEAF_CELL_ZOOM);
    let lap = 1i64 << LEAF_CELL_ZOOM;
    let (lo, hi) = (
        |v: f64| libm::floor(v / leaf) as i64,
        |v: f64| libm::ceil(v / leaf) as i64,
    );
    let b = [
        lo(around.min[0]),
        lo(around.min[1]),
        hi(around.max[0]),
        hi(around.max[1]),
    ];
    let shift = (b[0] - i64::from(e[0])).div_euclid(lap) * lap;
    let ys = i64::from(e[1]) < b[3] && b[1] < i64::from(e[3]);
    ys && [shift, shift + lap]
        .iter()
        .any(|s| i64::from(e[0]) + s < b[2] && b[0] < i64::from(e[2]) + s)
}

fn drawing(p: &mut [u32; drawn::DRAWING_WORDS as usize], seen: Key) {
    let anchor = seen.anchor;
    p.fill(0);
    p[drawn::MODEL_ANGLE as usize] = (decor_model_angle() as f32).to_bits();
    p[drawn::IMPOSTOR_ANGLE as usize] = (decor_impostor_angle() as f32).to_bits();
    let at = drawn::ANCHOR as usize;
    p[at] = wrap(anchor[0] as i32);
    p[at + 1] = anchor[1] as i32 as u32;
    p[at + 2] = anchor[2] as i32 as u32;
    if let Some([x, y, z]) = seen.scattered_cell {
        let at = drawn::HIDE as usize;
        p[at..at + 4].copy_from_slice(&[wrap(x), y as u32, z as u32, 1]);
    }
    let extents = DECORS
        .iter()
        .flat_map(|d| d.variants.iter().map(|v| v.extent(catalog()) as f32));
    for (k, e) in extents.enumerate() {
        p[drawn::EXTENTS as usize + k] = e.to_bits();
    }
}

/// Holds the regions where objects were written (domain/query/changes.rs) for
/// the next frame to hand. Each is kept as its columns in leaf cells,
/// `[x0, y0, x1, y1)`, with X on whatever lap it was written on. Past
/// `object::EDITS` regions, the rest are merged into the last one, so no
/// write is lost however many turns pass before a frame takes them.
pub fn note_edited(regions: &[Aabb]) {
    let h = hand_over();
    let leaf = units(LEAF_CELL_ZOOM);
    for r in regions {
        let b = [
            libm::floor(r.min[0] / leaf),
            libm::floor(r.min[1] / leaf),
            libm::ceil(r.max[0] / leaf),
            libm::ceil(r.max[1] / leaf),
        ]
        .map(|v| v as i32);
        if h.edited < h.edits.len() {
            h.edits[h.edited] = b;
            h.edited += 1;
        } else {
            let last = &mut h.edits[h.edited - 1];
            *last = [
                last[0].min(b[0]),
                last[1].min(b[1]),
                last[2].max(b[2]),
                last[3].max(b[3]),
            ];
        }
    }
}

/// The hand-over `step` built. The edits `note_edited` held are handed once
/// and then cleared. With the decor setting off, the ground generates its
/// decor at no zoom, while placed decor is still drawn.
pub fn frame() -> DecorFrame<'static> {
    let h = hand_over();
    let edited = core::mem::take(&mut h.edited);
    DecorFrame {
        zooms: if settings_store().decor {
            &h.shown
        } else {
            &[0; DECORS.len()]
        },
        drawing: &h.drawing,
        placed_records: &h.placed,
        anchor: h.anchor,
        edits: &h.edits[..edited],
    }
}

#[cfg(test)]
#[path = "gpu.test.rs"]
mod test;
