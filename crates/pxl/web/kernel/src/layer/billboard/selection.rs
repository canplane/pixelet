// pxl-kernel/layer/billboard/selection.rs

//! Shared visibility selection for world-anchored billboards (tier 3 — see
//! issue/20260907-world-rendering-tiers.md). A consumer (POI today; a future
//! chat-bubble-per-character feature would hit the same shape) calls the
//! selection once per placement:
//!
//! 1. Real camera-frustum membership, tested against a sphere around the
//!    anchor rather than the bare anchor point. The sphere matters at an
//!    oblique camera: a billboard is drawn *above* its anchor on screen, so an
//!    anchor that has just dropped below the bottom edge still has its pill in
//!    full view — and at an oblique polar the bottom edge is where the
//!    *nearest* anchors are. Point membership therefore deleted the closest
//!    badges first while distant ones near the horizon stayed.
//! 2. The fade and the viewport: what has faded past legibility, or whose box
//!    does not reach the screen, is not drawn.
//!
//! What survives is every candidate, overlapping or not: a badge stands for
//! the places under its cell, and hiding one behind another would drop them
//! from what the screen counts. They are listed nearest what the view looks
//! at first, a stable hash of each candidate's own id breaking ties, which is
//! the order a list of them is read in.

use pxl_spec::space::mat4::mat4_transform_point;

use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::distance_fade::distance_fade_far;
use crate::layer::environment::state::globe;
use crate::world::frame::view::{globe_anchor_local, view, view_sees_sphere};

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

/// The candidates that survive the frustum, the viewport and the fade, as
/// indices into `batch` written to `out`. Indices rather than items: this
/// pass never learns what a candidate *is*. The batch's coordinates are in the
/// root frame (world/frame/view.rs).
pub fn select_visible_billboards(batch: &mut BillboardBatch, viewport_w: f64, viewport_h: f64, out: &mut Vec<u32>) {
    let v = view();
    let eye = v.pose.eye;
    let anchor = globe_anchor_local();

    let mut n = 0usize;
    let globe = globe();
    for i in 0..batch.count as usize {
        // Onto the planet about its anchor, as badge.wgsl places it, gives
        // the point the frustum test needs, the point the distance is
        // measured from, and the point the screen box is projected from —
        // done once, and kept in the lanes.
        let from = [batch.local_x[i] - anchor[0], batch.local_y[i] - anchor[1], batch.local_z[i] - anchor[2]];
        let placed = globe.place(from);
        let pos = [anchor[0] + placed[0], anchor[1] + placed[1], anchor[2] + placed[2]];
        let dist2 = (pos[0] - eye[0]).powi(2) + (pos[1] - eye[1]).powi(2) + (pos[2] - eye[2]).powi(2);
        if !view_sees_sphere(pos[0], pos[1], pos[2], dist2.sqrt() * ANCHOR_REACH) {
            continue;
        }
        batch.world_x[i] = pos[0];
        batch.world_y[i] = pos[1];
        batch.world_z[i] = pos[2];
        batch.d2[i] = dist2;
        batch.order[n] = i as u32;
        n += 1;
    }

    // Nearest the eye first, which is how the pyramid orders what it asks
    // for too (world/pyramid.rs). Squared, since only the order matters; the
    // tiebreak decides nothing on its own and only keeps two at equal
    // distance from swapping frame to frame.
    let at = |i: u32| batch.d2[i as usize];
    let tiebreak = &batch.tiebreak;
    batch.order[..n].sort_by(|&a, &b| at(a).total_cmp(&at(b)).then(tiebreak[a as usize].total_cmp(&tiebreak[b as usize])));

    out.clear();
    let mut ndc = [0.0; 3];

    for k in 0..n {
        let i = batch.order[k] as usize;

        let far = distance_fade_far(batch.d2[i].sqrt());
        // Not `far <= 0`. The last stretch before the fade's end is ten pixels by
        // three at a tenth opacity — invisible, and not worth a draw.
        if far < MIN_LEGIBLE_FAR {
            continue;
        }

        mat4_transform_point(&mut ndc, &v.local_view_proj, batch.world_x[i], batch.world_y[i], batch.world_z[i]);
        let sx = (ndc[0] * 0.5 + 0.5) * viewport_w;
        let sy = (-ndc[1] * 0.5 + 0.5) * viewport_h;

        // Canvas Y and screen Y both grow downward, so the anchor's canvas offset
        // reads straight across: the box's top sits that far above the
        // projected anchor, and its bottom is the anchor.
        let w = batch.w_px[i] as f64 * far;
        let x0 = sx - w / 2.0;
        let y0 = sy - batch.anchor_px[i] as f64 * far;
        let (x1, y1) = (x0 + w, sy);
        // The frustum test keeps an anchor a margin past the screen's edge,
        // since its sprite can reach in. One whose box does not reach the
        // viewport at all is not shown.
        if x1 < 0.0 || y1 < 0.0 || x0 > viewport_w || y0 > viewport_h {
            continue;
        }
        batch.screen[i] = [x0, y0, x1, y1];
        out.push(i as u32);
    }
}
