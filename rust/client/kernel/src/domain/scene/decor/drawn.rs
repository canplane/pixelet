// pxl-kernel/domain/scene/decor/drawn.rs

//! What the GPU's decor takes each frame (pxl-render gpu/generated.rs), which
//! generates what the ground generates and draws it with placed decor: what
//! only the CPU knows, handed over and never read back. Each kind's zooms
//! where it is an impostor's worth, the tiers' floors as angles, the anchor
//! a decor is drawn about, the decor the eraser leaves out, each
//! variant's extent, placed decor as records, and the boxes the objects
//! were written in, where what the ground generates may have changed.

use pxl_render::gpu::generated::DecorFrame;
use pxl_spec::asset::decor::DECORS;
use pxl_spec::space::camera::grid::{split_dist, WINDOW_CELLS_A_RAD, WINDOW_MARGIN};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{idx_of, units, Zoom, UNIT_ZOOM};
use pxl_spec_core::generated::{self, drawn, row_of};
use pxl_spec_core::overwrite;
use pxl_spec_core::scatter;
use pxl_spec_core::LEAF_CELL;
use xpute_guest::global::Global;

use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::query::World;
use crate::domain::scene::decor::param;
use crate::domain::view::setting::store::settings_store;
use crate::sys::io::loader::catalog;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::camera::grid;
use pxl_spec::space::layer::lattice;
use pxl_spec::space::layer::tile;
use pxl_spec_core::decor;
use pxl_spec_core::window::FIRST_CELL;

/// A decor is drawn as a model down to `DECOR_MODEL_CELLS` of a camera
/// cell, the model's edge, an angle about the eye in radians (pxl-spec
/// space/camera/grid.rs `CELL`), so no screen's size or ratio moves it. No fade: the edge is per instance,
/// so it staggers.
pub fn decor_model_angle() -> f64 {
    param::DECOR_MODEL_CELLS * grid::CELL
}

/// The impostor's edge, `DECOR_IMPOSTOR_CELLS` of a camera cell, as an angle as
/// the model's is (`decor_model_angle`).
pub fn decor_impostor_angle() -> f64 {
    param::DECOR_IMPOSTOR_CELLS * grid::CELL
}

/// How far a thing `extent_u` wide is an impostor's worth as a window holds
/// it, in units, past which it is culled: where it spans
/// `DECOR_IMPOSTOR_CELLS` of the window's cells (`WINDOW_CELLS_A_RAD`),
/// `WINDOW_MARGIN` times as far as `decor_impostor_angle` puts the
/// impostor's edge, an upper bound. `is_decor_shown` takes both its
/// sides at the window's, so its zooms are the view's.
pub const fn cull_dist_u(extent_u: f64) -> f64 {
    extent_u * WINDOW_CELLS_A_RAD / param::DECOR_IMPOSTOR_CELLS
}

/// Whether a decor `extent_u` wide is an impostor's worth anywhere cells of
/// `cell_zoom` are drawn: as it spans where that zoom begins.
fn is_decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    split_dist(cell_zoom) * WINDOW_MARGIN <= cull_dist_u(extent_u)
}

/// Each kind's window zooms, a bit each, where it is an impostor's worth: the
/// leaves', and each coarser one where one of its variants stands as itself,
/// its own size no narrower than the window's cell (pxl-spec-core
/// `scatter::stands_at`), the same cells' decor read at every one. A finer
/// window lies within the leaves'.
fn shown() -> [u32; DECORS.len()] {
    shown_by(|d| d.extent(catalog()))
}

/// `shown`, each kind's extent `extent` gives.
fn shown_by(extent: impl Fn(&Decor) -> f64) -> [u32; DECORS.len()] {
    core::array::from_fn(|k| {
        let (d, extent) = (&DECORS[k], extent(&DECORS[k]));
        (FIRST_CELL..=LEAF_CELL)
            .filter(|&cell| {
                let stands = (0..d.row.variants).any(|v| scatter::stands_at(d.row, v, cell));
                stands && is_decor_shown(extent, cell)
            })
            .fold(0, |bits, cell| bits | 1 << (cell - FIRST_CELL))
    })
}

/// The frame's hand-over, kept so a settled frame asks the heap for nothing,
/// and made again only where what it is made of changed.
struct Handed {
    /// Made with the drawing words, from the variants' extents.
    shown: [u32; DECORS.len()],
    drawing: [u32; drawn::DRAWING_WORDS as usize],
    placed: Vec<[u32; generated::RECORD_WORDS as usize]>,
    /// The boxes the objects were written in at the last take, in leaf
    /// cells, and how many the next frame hands (`note_edited`, `frame`).
    edits: [[i32; 4]; overwrite::EDITS as usize],
    edited: usize,
    translation: [f32; 3],
    /// What the drawing words were last made of, by value.
    seen: Option<Seen>,
    /// The cell the placed decor was last walked about, and what it left out.
    walked: Option<Walked>,
}

/// What the drawing words are made of: the unit the eye stands in (its
/// anchor), the ground's decor the eraser leaves out, and whether the
/// catalog that measures the variants is read. A new input goes here or the
/// drawing words never see it change. The eye within its unit moves only
/// the translation.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    anchor: V3,
    scattered: Option<[i32; 3]>,
    read: bool,
}

/// What the placed decor was walked over: the eye's cell at the reach's
/// zoom, the coarsest cell no wider than the reach is short of, and the
/// placed decor the eraser leaves out. Walked again where this moves, or
/// where a write meets its box (`note_edited`): a traversal kept while the eye
/// stays in its cell.
#[derive(Clone, Copy, PartialEq)]
struct Walked {
    zoom: Zoom,
    cell: [i32; 3],
    hide: Option<EntryAt>,
}

static HANDED: Global<Handed> = Global::new();

fn handed() -> &'static mut Handed {
    HANDED.get(|| Handed {
        shown: [0; DECORS.len()],
        drawing: [0; drawn::DRAWING_WORDS as usize],
        placed: Vec::new(),
        edits: [[0; 4]; overwrite::EDITS as usize],
        edited: 0,
        translation: [0.0; 3],
        seen: None,
        walked: None,
    })
}

/// A leaf cell's X as the world names it (pxl-spec-core `wrap_idx`), the
/// GPU's generated records' among them, as a word.
fn wrapped(x: i32) -> u32 {
    pxl_spec_core::wrap_idx(LEAF_CELL, x) as u32
}

/// This frame's hand-over for an eye at `eye`: placed decor out to where the
/// largest is an impostor's worth or the rays stop asking the leaves' cells,
/// less the placed decor `hide` names, and the
/// ground's decor at `scattered` left out.
pub fn step(eye: V3, hide: Option<EntryAt>, scattered: Option<[i32; 3]>) {
    let h = handed();
    let anchor = eye.map(libm::floor);
    h.translation = [0, 1, 2].map(|k| (anchor[k] - eye[k]) as f32);
    let seen = Seen {
        anchor,
        scattered,
        read: catalog().is_some(),
    };
    if h.seen != Some(seen) {
        h.seen = Some(seen);
        drawing(&mut h.drawing, seen);
        h.shown = shown();
    }
    // Out to the reach from anywhere in the anchor's unit: every unit of the
    // eye's cell at the reach's zoom reaches no farther than the cell and
    // one beside it, whose side the reach is short of. What is past an
    // eye's own reach `decor_cnt_cs` leaves out by its size, as it does
    // what is past the box.
    // No farther than the rays ask the leaves' cells, past which none is drawn.
    let reach = (DECORS
        .iter()
        .map(|d| d.extent(catalog()))
        .fold(0.0, f64::max)
        / decor_impostor_angle())
    .min(lattice::draw_reach(lattice::GROUND, tile::LEAF_ZOOM));
    let mut zoom = UNIT_ZOOM;
    while zoom > 0 && units(zoom) < reach {
        zoom -= 1;
    }
    let side = units(zoom);
    let cell = anchor.map(|v| libm::floor(v / side) as i32);
    let around = Aabb {
        min: [0, 1, 2].map(|k| (cell[k] - 1) as f64 * side),
        max: [0, 1, 2].map(|k| (cell[k] + 2) as f64 * side),
    };
    let walked = Walked { zoom, cell, hide };
    let met = h.edits[..h.edited].iter().any(|e| meets(e, &around));
    if h.walked == Some(walked) && !met {
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
            generated::record_word(k, variant, turn, generated::PLACED_AT),
            wrapped(x),
            y as u32,
            z as u32,
            0,
        ]);
    });
}

/// Whether an edit's columns, `[x0, y0, x1, y1)` in leaf cells, X on its own
/// lap, meet `around`'s on any lap.
fn meets(e: &[i32; 4], around: &Aabb) -> bool {
    let leaf = units(LEAF_CELL);
    let lap = 1i64 << LEAF_CELL;
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

/// The drawing words of a hand-over `seen` makes.
fn drawing(p: &mut [u32; drawn::DRAWING_WORDS as usize], seen: Seen) {
    let anchor = seen.anchor;
    p.fill(0);
    p[drawn::MODEL_ANGLE as usize] = (decor_model_angle() as f32).to_bits();
    p[drawn::IMPOSTOR_ANGLE as usize] = (decor_impostor_angle() as f32).to_bits();
    let at = drawn::ANCHOR as usize;
    p[at] = wrapped(anchor[0] as i32);
    p[at + 1] = anchor[1] as i32 as u32;
    p[at + 2] = anchor[2] as i32 as u32;
    if let Some([x, y, z]) = seen.scattered {
        let at = drawn::HIDE as usize;
        p[at..at + 4].copy_from_slice(&[wrapped(x), y as u32, z as u32, 1]);
    }
    let extents = DECORS
        .iter()
        .flat_map(|d| d.variants.iter().map(|v| v.extent(catalog()) as f32));
    for (k, e) in extents.enumerate() {
        p[drawn::EXTENTS as usize + k] = e.to_bits();
    }
}

/// Where the objects were written, as the turn's change took them
/// (domain/query/changed.rs), held for the next frame to hand: each region's
/// columns in leaf cells, `[x0, y0, x1, y1)`, X on its own lap; past
/// `overwrite::EDITS`, the rest joined into the last, so every write is met,
/// however many turns pass before a frame takes them.
pub fn note_edited(regions: &[Aabb]) {
    let h = handed();
    let leaf = units(LEAF_CELL);
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

/// The hand-over `step` made; the edits `note_edited` gave, once. With the
/// decor setting off the ground generates its decor at no zoom, and what was
/// placed stays.
pub fn frame() -> DecorFrame<'static> {
    let h = handed();
    let edited = core::mem::take(&mut h.edited);
    DecorFrame {
        shown: if settings_store().decor {
            &h.shown
        } else {
            &[0; DECORS.len()]
        },
        drawing: &h.drawing,
        placed: &h.placed,
        translation: h.translation,
        edits: &h.edits[..edited],
    }
}

#[cfg(test)]
#[path = "drawn.test.rs"]
mod test;
