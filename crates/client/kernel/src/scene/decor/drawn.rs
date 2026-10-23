// pxl-kernel/scene/decor/drawn.rs

//! What the GPU's decor takes each frame (pxl-render gpu/grown.rs), which
//! grows what the ground grows and draws it with placed decor: what only
//! the CPU knows, handed over and never read back. Each kind's zooms where
//! it is a sprite's worth, a pixel's size and the tiers' floors, the anchor
//! the decor block is drawn about, the decor the eraser leaves out, each
//! variant's extent, and placed decor as records.

use pxl_portable::scatter::{drawn, row_of};
use pxl_portable::LEAF_CELL;
use pxl_render::gpu::grown::DecorFrame;
use pxl_spec::block::decor::DECORS;
use pxl_spec::space::camera::grid::{split_distance, target_cell_px, SPLIT};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{index_of, Zoom, UNIT_ZOOM};
use xpute_guest::global::Global;

use crate::layer::object::meta::user_decor_pick;
use crate::layer::object::store::{EntryAt, Store};
use crate::setting::store::settings_store;
use crate::view::frame::pose::units_per_px_at;
use pxl_portable::window::FIRST_CELL;

/// A decor is drawn as a model down to `DECOR_MODEL_PX`, then as a sprite
/// down to `DECOR_SPRITE_CELLS` of a camera cell. No fade: the edge is per
/// instance, so it staggers.
pub fn decor_gone_px() -> f64 {
    crate::param::DECOR_MODEL_PX
}

/// The sprite's edge in pixels, at the reference view's cell.
pub fn decor_sprite_px() -> f64 {
    crate::param::DECOR_SPRITE_CELLS * target_cell_px()
}

/// How far a thing `extent_u` wide is a sprite's worth: where it spans
/// `DECOR_SPRITE_CELLS` of the camera grid's cells, an angle about the eye
/// whatever the screen.
pub const fn sprite_reach(extent_u: f64) -> f64 {
    extent_u * SPLIT / crate::param::DECOR_SPRITE_CELLS
}

/// Whether a decor `extent_u` wide is a sprite's worth anywhere cells of
/// `cell_zoom` are drawn: as it spans where that zoom begins.
fn decor_shown(extent_u: f64, cell_zoom: Zoom) -> bool {
    split_distance(cell_zoom) <= sprite_reach(extent_u)
}

/// The frame's hand-over, kept so a settled frame asks the heap for nothing,
/// and made again only where what it is made of changed.
struct Handed {
    on: bool,
    /// A constant of the decor table, made once.
    shown: [u32; DECORS.len()],
    params: [u32; drawn::PARAM_WORDS as usize],
    placed: Vec<[u32; 4]>,
    translation: [f32; 3],
    /// What it was last made of, by value.
    seen: Option<Seen>,
    /// Moves each time it is made again: what the renderer reads to know
    /// the hand-over changed.
    revision: u32,
}

/// Everything the hand-over is made of: the eye, a pixel's size there, what
/// the eraser leaves out, and the placed objects as the store's revision
/// counts them. A new input goes here or the hand-over never sees it change.
#[derive(Clone, Copy, PartialEq)]
struct Seen {
    eye: V3,
    upp: f64,
    hide: Option<EntryAt>,
    scattered: Option<[i64; 3]>,
    objects: u32,
}

static HANDED: Global<Handed> = Global::new();

fn handed() -> &'static mut Handed {
    HANDED.get(|| Handed {
        on: false,
        shown: core::array::from_fn(|k| (FIRST_CELL..=LEAF_CELL).filter(|&c| decor_shown(DECORS[k].extent(), c)).fold(0, |w, c| w | 1 << (c - FIRST_CELL))),
        params: [0; drawn::PARAM_WORDS as usize],
        placed: Vec::new(),
        translation: [0.0; 3],
        seen: None,
        revision: 0,
    })
}

/// Leaf cells round the world, as the GPU's X wraps.
const ROUND: i64 = 1 << LEAF_CELL;

/// This frame's hand-over for an eye at `eye`: placed decor out to where the
/// largest is a sprite's worth, less the placed decor `hide` names, and the
/// ground's decor at `scattered` left out.
pub fn step(store: &Store, eye: V3, hide: Option<EntryAt>, scattered: Option<[i64; 3]>) {
    let h = handed();
    h.on = settings_store().decor;
    if !h.on {
        h.seen = None;
        return;
    }
    let seen = Seen {
        eye,
        upp: units_per_px_at(1.0),
        hide,
        scattered,
        objects: store.raw_lanes().3,
    };
    if h.seen == Some(seen) {
        return;
    }
    h.seen = Some(seen);
    h.revision = h.revision.wrapping_add(1);
    let anchor = eye.map(libm::floor);
    let p = &mut h.params;
    p.fill(0);
    p[drawn::UPP as usize] = (seen.upp as f32).to_bits();
    p[drawn::MODEL_PX as usize] = (decor_gone_px() as f32).to_bits();
    p[drawn::SPRITE_PX as usize] = (decor_sprite_px() as f32).to_bits();
    let at = drawn::ANCHOR as usize;
    p[at] = (anchor[0] as i64).rem_euclid(ROUND) as u32;
    p[at + 1] = anchor[1] as i64 as u32;
    p[at + 2] = anchor[2] as i64 as u32;
    if let Some([x, y, z]) = scattered {
        let at = drawn::HIDE as usize;
        p[at..at + 4].copy_from_slice(&[x.rem_euclid(ROUND) as u32, y as u32, z as u32, 1]);
    }
    let extents = DECORS.iter().flat_map(|d| d.variants.iter().map(|v| v.extent() as f32));
    for (k, e) in extents.enumerate() {
        p[drawn::EXTENTS as usize + k] = e.to_bits();
    }
    h.translation = [0, 1, 2].map(|k| (anchor[k] - eye[k]) as f32);
    let reach = DECORS.iter().map(|d| d.extent()).fold(0.0, f64::max) / (decor_sprite_px() * seen.upp);
    let around = Aabb {
        min: eye.map(|v| v - reach),
        max: eye.map(|v| v + reach),
    };
    h.placed.clear();
    let placed = &mut h.placed;
    store.entries_in(&around, &mut |e| {
        let (k, decor) = row_of(&pxl_portable::decor::DECORS, e.state);
        if !decor || hide == Some(e.id()) {
            return;
        }
        let (variant, turn) = user_decor_pick(&e.corner(), &DECORS[k as usize]);
        let [x, y, z] = index_of(UNIT_ZOOM, &e.corner());
        placed.push([k | variant << 8 | turn << 16, x.rem_euclid(ROUND) as u32, y as u32, z as u32]);
    });
}

/// Moves each time the hand-over is made again, so it is the same while this is.
pub fn revision() -> u32 {
    handed().revision
}

/// The hand-over `step` made, none where decor is off.
pub fn frame() -> Option<DecorFrame<'static>> {
    let h = handed();
    h.on.then_some(DecorFrame {
        shown: &h.shown,
        params: &h.params,
        placed: &h.placed,
        translation: h.translation,
    })
}
