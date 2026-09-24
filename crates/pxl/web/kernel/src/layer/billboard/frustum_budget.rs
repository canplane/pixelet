// pxl-kernel/layer/billboard/frustum_budget.rs

//! Shared visibility budget for world-anchored billboards (tier 3 — see
//! issue/20260907-world-rendering-tiers.md). A consumer with more candidates
//! than it can afford to draw (POI today; a future chat-bubble-per-character
//! feature would hit the same shape) calls the selection once per placement:
//!
//! 1. Real camera-frustum membership, tested against a sphere around the
//!    anchor rather than the bare anchor point. The sphere matters at an
//!    oblique camera: a billboard is drawn *above* its anchor on screen, so an
//!    anchor that has just dropped below the bottom edge still has its pill in
//!    full view — and at an oblique polar the bottom edge is where the
//!    *nearest* anchors are. Point membership therefore deleted the closest
//!    badges first while distant ones near the horizon stayed.
//! 2. If still over max_count: nearest first, with a stable hash of each
//!    candidate's own id breaking ties.
//!
//! Distance restores monotonicity: widening the view can only append farther
//! candidates, so the ones already on screen keep their slots and what falls
//! off the end is the farthest — which is also the smallest and the least
//! legible.

use pxl_spec::space::lattice::grid_index::{GridIndex, Placement, DEFAULT_CELL_PX};
use xpute_core::math::mat4::mat4_transform_point;
use xpute_core::math::vec3::Vec3;

use crate::global::Global;
use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::distance_fade::distance_fade_far;
use crate::layer::environment::state::globe;
use crate::world::frame::view::{view, view_sees_sphere};

/// Placed badges in screen pixels, so a candidate can find out whether its
/// pixels are already taken. Sized once for a viewport larger than anything
/// this will meet; `begin` takes the live size, so a resize costs nothing.
const MAX_VIEWPORT_PX: u32 = 4096;

/// How far around its anchor a billboard can reach, as a fraction of its own
/// distance from the camera. A fraction rather than a world length because the
/// badge shader holds a billboard at a roughly constant *screen* size, so its
/// world extent grows with distance. Generous on purpose, since the cost of
/// keeping one badge too many is one sprite and the cost of dropping one too
/// early is the bug above.
const ANCHOR_REACH: f64 = 0.08;

/// The fade level below which a billboard is not worth a draw call. `far` is
/// the drawn size as a fraction of full and it is also the opacity, so this is
/// one number saying both "a tenth of the size" and "a tenth of the alpha".
/// Cutting here removes the outer shell of the reach and changes nothing anyone
/// can see.
const MIN_LEGIBLE_FAR: f64 = 0.12;

struct Selection {
    screen: GridIndex,
}

static SELECTION: Global<Selection> = Global::new();

fn selection() -> &'static mut Selection {
    SELECTION.get(|| Selection {
        screen: GridIndex::new(MAX_VIEWPORT_PX, MAX_VIEWPORT_PX, 512, DEFAULT_CELL_PX, 8),
    })
}

/// The candidates that survive the frustum, the budget and the collision pass,
/// as indices into `batch` written to `out`. Indices rather than items: this
/// pass never learns what a candidate *is*. The batch's coordinates are in the
/// root frame (world/frame/view.rs).
pub fn select_visible_billboards(batch: &mut BillboardBatch, max_count: u32, viewport_w: f64, viewport_h: f64, out: &mut [u32]) -> u32 {
    let sel = selection();
    let v = view();
    let eye = v.eye;

    let mut n = 0usize;
    let mut pos = Vec3::default();
    let globe = globe();
    for i in 0..batch.count as usize {
        // Through the root and onto the planet, as badge.wgsl places it,
        // gives the point the frustum test needs, the point the distance is
        // measured from, and the point the screen box is projected from —
        // done once, and kept in the lanes.
        mat4_transform_point(&mut pos, &v.root, batch.local_x[i], batch.local_y[i], batch.local_z[i]);
        let [x, y, z] = globe.place_turned([pos.x, pos.y, pos.z], v.root[1], v.root[0]);
        let pos = Vec3 { x, y, z };
        let dist2 = (pos.x - eye.x).powi(2) + (pos.y - eye.y).powi(2) + (pos.z - eye.z).powi(2);
        if !view_sees_sphere(pos.x, pos.y, pos.z, dist2.sqrt() * ANCHOR_REACH) {
            continue;
        }
        batch.world_x[i] = pos.x;
        batch.world_y[i] = pos.y;
        batch.world_z[i] = pos.z;
        batch.d2[i] = dist2;
        batch.order[n] = i as u32;
        n += 1;
    }

    // Squared distance: only the order matters. The tiebreak decides nothing on
    // its own — it keeps two candidates at equal distance from swapping places
    // frame to frame. Distance stays the ordering because it is what makes
    // widening the view append rather than reshuffle.
    let (d2, tiebreak) = (&batch.d2, &batch.tiebreak);
    batch.order[..n].sort_by(|&a, &b| d2[a as usize].total_cmp(&d2[b as usize]).then(tiebreak[a as usize].total_cmp(&tiebreak[b as usize])));

    // Rectangles outside this grid share its edge cells; their actual screen
    // bounds still decide collision. This keeps unusually wide displays safe.
    sel.screen.begin(viewport_w.min(MAX_VIEWPORT_PX as f64), viewport_h.min(MAX_VIEWPORT_PX as f64));
    let mut kept = 0;
    let mut ndc = Vec3::default();

    for k in 0..n {
        let i = batch.order[k] as usize;

        let far = distance_fade_far(batch.d2[i].sqrt());
        // Not `far <= 0`. The last stretch before the fade's end is ten pixels by
        // three at a tenth opacity — invisible, and small enough that hundreds
        // of them fit without ever colliding. This is the difference between a
        // budget bounded by what the screen can show and one bounded by how much
        // world is in reach.
        if far < MIN_LEGIBLE_FAR {
            continue;
        }

        mat4_transform_point(&mut ndc, &v.view_proj, batch.world_x[i], batch.world_y[i], batch.world_z[i]);
        let sx = (ndc.x * 0.5 + 0.5) * viewport_w;
        let sy = (-ndc.y * 0.5 + 0.5) * viewport_h;

        // Canvas Y and screen Y both grow downward, so the anchor's canvas offset
        // reads straight across: the box's top sits that far above the projected
        // anchor, and it is `collide_px` tall rather than the full drawn height.
        let w = batch.w_px[i] as f64 * far;
        let x0 = sx - w / 2.0;
        let y0 = sy - batch.anchor_px[i] as f64 * far;
        let (x1, y1) = (x0 + w, y0 + batch.collide_px[i] as f64 * far);

        let room = kept < max_count && (kept as usize) < out.len();
        let blocker = if room { sel.screen.place(x0, y0, x1, y1) } else { sel.screen.first_hit(x0, y0, x1, y1) };

        if room && matches!(blocker, Placement::Placed) {
            out[kept as usize] = i as u32;
            kept += 1;
        }
    }
    kept
}
