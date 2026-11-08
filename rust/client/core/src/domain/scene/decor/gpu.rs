// pxl-core/domain/scene/decor/gpu.rs

//! What the GPU's decor takes each frame (pxl-render gpu/generator.rs), which
//! generates what the ground generates and draws it with placed decor: what
//! only the CPU knows, handed over and never read back. Each kind's zooms
//! the ground generates it at and the tiers' floors as angles, the lens's
//! (domain/view/lens.rs), the anchor a decor is drawn about, the decor the
//! eraser leaves out, each variant's extent, placed decor as records, and the
//! boxes the objects were written in, where what the ground generates may
//! have changed.

use pxl_content::generation::{self, drawn, row_of};
use pxl_content::scatter;
use pxl_engine::overwrite;
use pxl_engine::LEAF_CELL;
use pxl_render::gpu::api;
use pxl_render::gpu::draw::Position;
use pxl_render::gpu::generator::DecorFrame;
use pxl_spec::asset::decor::DECORS;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{idx_of, units, Zoom, UNIT_ZOOM};
use xpute_core::global::Global;

use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::preview::RenderEdits;
use crate::domain::query::World;
use crate::domain::view::lens::{decor_impostor_angle, decor_model_angle, decor_zooms};
use crate::sys::io::loader::catalog;
use pxl_content::decor;
use pxl_spec::space::layer::lattice;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant;

/// The frame's hand-over, kept so a settled frame asks the heap for nothing,
/// and made again only where what it is made of changed.
struct Frame {
    /// Each kind's zooms as this frame hands them (`decor_zooms`).
    zooms: [u32; DECORS.len()],
    /// The zoom placed decor is walked at (`walk_zoom`), made with the
    /// drawing words.
    walk_zoom: Zoom,
    drawing: [u32; drawn::DRAWING_WORDS as usize],
    placed: Vec<[u32; generation::RECORD_WORDS as usize]>,
    /// The boxes the objects were written in at the last take, in leaf
    /// cells, and how many the next frame hands (`note_edited`, `frame`).
    edits: [[i32; 4]; overwrite::EDITS as usize],
    edited: usize,
    /// The unit the eye stands in, the lanes' anchor, as a draw's Q3
    /// (pxl-render gpu/draw.rs `Position`).
    anchor: Q3,
    /// What the drawing words were last made of, by value.
    seen: Option<Key>,
    /// The cell the placed decor was last walked about, and what it left out.
    walked: Option<WalkKey>,
}

/// What the drawing words are made of: the unit the eye stands in (its
/// anchor), the ground's decor the eraser leaves out, and whether the
/// catalog that measures the variants is read. A new input goes here or the
/// drawing words never see it change. The eye within its unit moves nothing
/// here, only the frame's eye (pxl-render word.rs `frame::EYE`).
#[derive(Clone, Copy, PartialEq)]
struct Key {
    anchor: V3,
    scattered_cell: Option<[i32; 3]>,
    read: bool,
}

/// What the placed decor was walked over: the eye's cell at the reach's
/// zoom, the coarsest cell no wider than the reach is short of, and the
/// placed decor the eraser leaves out. Walked again where this moves, or
/// where a write meets its box (`note_edited`): a traversal kept while the eye
/// stays in its cell.
#[derive(Clone, Copy, PartialEq)]
struct WalkKey {
    zoom: Zoom,
    cell: [i32; 3],
    hide: Option<Octant>,
}

static HAND_OVER: Global<Frame> = Global::new();

fn hand_over() -> &'static mut Frame {
    HAND_OVER.get(|| Frame {
        zooms: [0; DECORS.len()],
        walk_zoom: UNIT_ZOOM,
        drawing: [0; drawn::DRAWING_WORDS as usize],
        placed: Vec::new(),
        edits: [[0; 4]; overwrite::EDITS as usize],
        edited: 0,
        anchor: [0; 3],
        seen: None,
        walked: None,
    })
}

/// A leaf cell's X as the world names it (pxl-engine `wrap_idx`), the
/// GPU's generated records' among them, as a word.
fn wrap(x: i32) -> u32 {
    pxl_engine::wrap_idx(LEAF_CELL, x) as u32
}

/// This frame's hand-over for an eye at `eye`: placed decor out to where the
/// largest is an impostor's worth or the rays stop asking the leaves' cells,
/// with the placed decor and the ground's decor the eraser's preview hides
/// left out (`edits`, as the turn hands it).
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
    // Only a decor is left out of the records: anything else hidden is the
    // rays' to cut, and the walk is kept.
    let hide = hide.filter(|&at| {
        World::now()
            .entry_at(at)
            .is_some_and(|e| e.asset().decor().is_some())
    });
    if h.seen != Some(seen) {
        h.seen = Some(seen);
        drawing(&mut h.drawing, seen);
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
    // The renderer hands the records where they lie: while the runtime holds
    // them they stay, walked again at a later frame (pxl-render gpu/api.rs
    // `in_flight`).
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

/// The zoom of the coarsest cell no wider than the reach placed decor is
/// walked out to is short of: out to where the largest is an impostor's
/// worth, and no farther than the rays ask the leaves' cells, past which
/// none is drawn. Every unit of the eye's cell at that zoom reaches no
/// farther than the cell and one beside it. What is past an eye's own
/// distance `decor_cnt_cs` leaves out by its size, as it does what is past
/// the box.
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

/// Where the objects were written, as the turn's change took them
/// (domain/query/changes.rs), held for the next frame to hand: each region's
/// columns in leaf cells, `[x0, y0, x1, y1)`, X on its own lap; past
/// `overwrite::EDITS`, the rest joined into the last, so every write is met,
/// however many turns pass before a frame takes them.
pub fn note_edited(regions: &[Aabb]) {
    let h = hand_over();
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

/// The hand-over `step` made, with each kind's zooms as they are now
/// (`decor_zooms`); the edits `note_edited` gave, once.
pub fn frame() -> DecorFrame<'static> {
    let h = hand_over();
    let edited = core::mem::take(&mut h.edited);
    h.zooms = decor_zooms();
    DecorFrame {
        zooms: &h.zooms,
        drawing: &h.drawing,
        placed_records: &h.placed,
        anchor: h.anchor,
        edits: &h.edits[..edited],
    }
}
