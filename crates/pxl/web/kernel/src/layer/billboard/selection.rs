// pxl-kernel/layer/billboard/selection.rs

//! Visibility of world-anchored billboards: in the frustum as a sphere about
//! the anchor (a point test dropped the nearest badges first at an oblique
//! camera, since a badge is drawn above its anchor), and with a box on
//! screen. Overlaps are kept: a badge stands for the places under its cell.

use pxl_spec::space::mat4::mat4_transform_point;
use pxl_spec::space::v3::{add3, dot3, sub3};

use crate::layer::billboard::batch::BillboardBatch;
use crate::layer::billboard::distance_fade::distance_fade_far;
use crate::layer::environment::state::globe;
use crate::world::frame::view::{globe_anchor_local, ndc_to_px, view, view_sees_sphere};

/// A fraction of the distance, since the badge holds a constant screen size.
/// Generous: one badge too many costs a sprite.
const ANCHOR_REACH: f64 = 0.08;

/// Indices into `batch`, in the root frame.
pub fn select_visible_billboards(batch: &mut BillboardBatch, out: &mut Vec<u32>) {
    let v = view();
    let eye = v.pose.eye;
    let anchor = globe_anchor_local();

    let mut n = 0usize;
    let globe = globe();
    for i in 0..batch.count as usize {
        // Placed on the planet once, as badge.wgsl does, and kept in the lanes.
        let from = [batch.local_x[i] - anchor[0], batch.local_y[i] - anchor[1], batch.local_z[i] - anchor[2]];
        let placed = globe.place(from);
        let pos = add3(&anchor, &placed);
        let to = sub3(&pos, &eye);
        let dist2 = dot3(&to, &to);
        if !v.frustum.sees(pos, dist2.sqrt() * ANCHOR_REACH) {
            continue;
        }
        batch.world_x[i] = pos[0];
        batch.world_y[i] = pos[1];
        batch.world_z[i] = pos[2];
        batch.d2[i] = dist2;
        batch.order[n] = i as u32;
        n += 1;
    }

    // Nearest first; the tiebreak only stops equal distances swapping frame to frame.
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
        let [sx, sy] = ndc_to_px([ndc[0], ndc[1]]);

        let w = batch.w_px[i] as f64;
        let x0 = sx - w / 2.0;
        let y0 = sy - batch.anchor_px[i] as f64 * far;
        let (x1, y1) = (x0 + w, sy);
        if x1 < 0.0 || y1 < 0.0 || x0 > v.width || y0 > v.height {
            continue;
        }
        batch.screen[i] = [x0, y0, x1, y1];
        out.push(i as u32);
    }
}
