// pxl-core/domain/scene/decor/gpu.rs

//! What the core hands the GPU's decor each frame (pxl-render
//! gpu/generator.rs). The GPU generates the decor the ground scatters and
//! draws it together with the decor users placed. The core hands it only what
//! the CPU alone knows, and never reads back what the GPU generated. Which
//! zooms each decor is generated at and the angles that set its lod come from
//! the lens (domain/view/lens.rs). Among what the core hands are the boxes
//! where objects were written since the last frame, because the ground's
//! generated decor may have changed there.

use pxl_content::generation::{self, drawing};
use pxl_content::scatter;
use pxl_engine::object;
use pxl_global::Global;
use pxl_render::gpu::api;
use pxl_render::gpu::draw::Position;
use pxl_render::gpu::generator::{DecorFrame, hold};
use pxl_spec::asset::decor::DECORS;
use pxl_spec::asset::id;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::{idx_of, wrap_idx};
use pxl_spec::space::coord::zoom::{LEAF_CELL_ZOOM, LEAF_ZOOM, UNIT_ZOOM, Zoom, units};

use crate::domain::layer::object::meta::user_decor_pick;
use crate::domain::preview::RenderEdit;
use crate::domain::query::World;
use crate::domain::setting::settings;
use crate::domain::view::lens::{decor_impostor_angle, decor_model_angle, decor_zooms};
use crate::sys::io::loader::catalog;
use pxl_spec::space::layer::lattice;
use pxl_spec::space::layer::octant::Octant;

/// What this module hands the GPU each frame. It is kept between frames, so a
/// frame where nothing changed allocates nothing, and each part but `zooms`
/// is rebuilt only when its inputs change.
struct Frame {
    /// Each decor's zooms (`decor_zooms`). They are taken again every frame,
    /// because the decor setting is not among the inputs `last_key` compares.
    zooms: [u32; DECORS.len()],
    walk_zoom: Zoom,
    drawing: [u32; drawing::WORDS as usize],
    recs: Vec<[u32; generation::REC_WORDS as usize]>,
    /// The damage: the column boxes, in leaf cells, where objects were written
    /// since a frame last took them (`note_damage`). The first `damage_cnt` of
    /// them are the ones the next frame hands.
    damage: [[i32; 4]; object::WRITE_DAMAGE_CAP as usize],
    damage_cnt: usize,
    /// The unit cell the eye stands in, as a draw's Q3 position (pxl-render
    /// gpu/draw.rs `Position`). The GPU draws the decor's lanes about it.
    anchor: Q3,
    /// The inputs `drawing` and `walk_zoom` were last built from.
    last_key: Option<Key>,
    /// What the last walk of placed decor was made over.
    last_walk: Option<WalkKey>,
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
/// walked box (`note_damage`), so one walk serves while the eye stays in its
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
        zooms: [0; DECORS.len()],
        walk_zoom: UNIT_ZOOM,
        drawing: [0; drawing::WORDS as usize],
        recs: Vec::new(),
        damage: [[0; 4]; object::WRITE_DAMAGE_CAP as usize],
        damage_cnt: 0,
        anchor: [0; 3],
        last_key: None,
        last_walk: None,
    })
}

/// A leaf cell's X wrapped as the world wraps it (pxl-spec `wrap_idx`), as a
/// word. The GPU's generated records name X the same way.
fn wrap(x: i32) -> u32 {
    wrap_idx(LEAF_CELL_ZOOM, x) as u32
}

/// Builds this frame's hand-over for an eye at `eye`. Placed decor is walked
/// out to the reach `walk_zoom` describes. The placed decor and the ground's
/// decor that the eraser's preview hides (`edit`) are left out.
pub(crate) fn step(eye: V3, edit: &RenderEdit) {
    let RenderEdit {
        hide,
        scattered_cell,
        ..
    } = *edit;
    let h = hand_over();
    let anchor = eye.map(libm::floor);
    h.anchor = Position::of(anchor).q;
    let key = Key {
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
    if h.last_key != Some(key) {
        h.last_key = Some(key);
        drawing(&mut h.drawing, key);
        h.walk_zoom = walk_zoom();
    }
    let zoom = h.walk_zoom;
    let side = units(zoom);
    let cell = anchor.map(|v| libm::floor(v / side) as i32);
    let around = Aabb {
        min: [0, 1, 2].map(|k| (cell[k] - 1) as f64 * side),
        max: [0, 1, 2].map(|k| (cell[k] + 2) as f64 * side),
    };
    let walk_key = WalkKey { zoom, cell, hide };
    let met = h.damage[..h.damage_cnt].iter().any(|e| meets(e, &around));
    if h.last_walk == Some(walk_key) && !met {
        return;
    }
    // The renderer hands the records to the host in place, without a copy.
    // While the host still holds them they must not change, so the walk
    // waits for a later frame (pxl-render gpu/api.rs `in_flight`).
    if api::in_flight(&h.recs) {
        h.last_walk = None;
        return;
    }
    h.last_walk = Some(walk_key);
    h.recs.clear();
    let recs = &mut h.recs;
    World::now().placed_in(&around, &mut |e| {
        let Some(decor) = e.asset().decor().filter(|_| hide != Some(e.id())) else {
            return;
        };
        let (variant, turn) = user_decor_pick(&e.corner(), decor);
        let [x, y, z] = idx_of(UNIT_ZOOM, &e.corner());
        recs.push([
            generation::rec_word(
                id::idx_of(decor.id),
                variant,
                turn,
                scatter::variant_first_cell_zoom(decor.row, variant),
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
    .min(lattice::draw_dist(lattice::GROUND, LEAF_ZOOM));
    let mut zoom = UNIT_ZOOM;
    while zoom > 0 && units(zoom) < dist {
        zoom -= 1;
    }
    zoom
}

/// Whether a box of damage, `[x0, y0, x1, y1)` in leaf cells, meets the
/// columns of `around`. X goes round the world, and the box's X is on whatever
/// lap it was written on, so the test tries it on every lap that could meet.
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

fn drawing(p: &mut [u32; drawing::WORDS as usize], key: Key) {
    let anchor = key.anchor;
    p.fill(0);
    p[drawing::MODEL_ANGLE as usize] = (decor_model_angle() as f32).to_bits();
    p[drawing::IMPOSTOR_ANGLE as usize] = (decor_impostor_angle() as f32).to_bits();
    let at = drawing::ANCHOR as usize;
    p[at] = wrap(anchor[0] as i32);
    p[at + 1] = anchor[1] as i32 as u32;
    p[at + 2] = anchor[2] as i32 as u32;
    if let Some([x, y, z]) = key.scattered_cell {
        let at = drawing::HIDE as usize;
        p[at..at + 4].copy_from_slice(&[wrap(x), y as u32, z as u32, 1]);
    }
    let extents = DECORS
        .iter()
        .flat_map(|d| d.variants.iter().map(|v| v.extent(catalog()) as f32));
    for (k, e) in extents.enumerate() {
        p[drawing::EXTENTS as usize + k] = e.to_bits();
    }
}

/// Holds the regions where objects were written (domain/query/change.rs) for
/// the next frame to hand. Each is kept as its columns in leaf cells,
/// `[x0, y0, x1, y1)`, with X on whatever lap it was written on. Past
/// `object::WRITE_DAMAGE_CAP` regions, the rest are joined into the last one
/// (pxl-render gpu/generator.rs `hold`), so no write is lost however many
/// writes come before a frame takes them.
pub(crate) fn note_damage(regions: &[Aabb]) {
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
        hold(&mut h.damage, &mut h.damage_cnt, b);
    }
}

/// The hand-over `step` built, with each decor's zooms as they are now
/// (`decor_zooms`). The damage `note_damage` held is handed once and then
/// cleared.
pub(crate) fn frame() -> DecorFrame<'static> {
    let h = hand_over();
    let damage_cnt = core::mem::take(&mut h.damage_cnt);
    h.zooms = decor_zooms(settings().decor);
    DecorFrame {
        zooms: &h.zooms,
        drawing: &h.drawing,
        placed_recs: &h.recs,
        anchor: h.anchor,
        damage: &h.damage[..damage_cnt],
    }
}
