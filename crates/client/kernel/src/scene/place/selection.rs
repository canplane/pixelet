// pxl-kernel/scene/place/selection.rs

//! Visibility of world-anchored billboards: in the frustum as a sphere about
//! the anchor (a point test dropped the nearest badges first at an oblique
//! camera, since a badge is drawn above its anchor), and with a box on
//! screen, nearest first.

use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::v3;

use crate::billboard::batch::BillboardBatch;
use crate::view::frame::pose::{ndc_to_px, view};

/// A fraction of the distance, since the badge holds a constant screen size.
/// Generous: one badge too many costs a sprite.
const ANCHOR_REACH: f64 = 0.08;

/// Indices into `batch`, in the root frame.
pub fn select_visible_billboards(batch: &mut BillboardBatch, out: &mut Vec<u32>) {
    let v = view();
    let eye = v.at().eye;

    let mut n = 0usize;
    for i in 0..batch.count as usize {
        // Where the rays find it: the world's own point, nothing bent.
        let pos = [batch.local_x[i], batch.local_y[i], batch.local_z[i]];
        let to = v3::sub(&pos, &eye);
        let dist2 = v3::dot(&to, &to);
        if !v.frustum.sees(pos, dist2.sqrt() * ANCHOR_REACH) {
            continue;
        }
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

        mat4::transform_point(&mut ndc, &v.world_view_proj, batch.local_x[i], batch.local_y[i], batch.local_z[i]);
        let [sx, sy] = ndc_to_px([ndc[0], ndc[1]]);

        let w = batch.region_px[i] as f64;
        let x0 = sx - w / 2.0;
        let y0 = sy - batch.anchor_px[i] as f64;
        let region = [x0, y0, x0 + w, y0 + batch.h_px[i] as f64];
        if region[2] < 0.0 || region[3] < 0.0 || x0 > v.width || y0 > v.height {
            continue;
        }
        batch.screen[i] = region;
        out.push(i as u32);
    }
}
