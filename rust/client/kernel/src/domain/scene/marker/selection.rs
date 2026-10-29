// pxl-kernel/domain/scene/marker/selection.rs

//! Visibility of world-anchored markers: in the frustum as a sphere about
//! the anchor (a point test dropped the nearest markers first at an oblique
//! camera, since a marker is drawn above its anchor), and with a box on
//! screen, nearest first.

use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::v3;

use crate::domain::scene::marker::candidate::Candidates;
use crate::domain::view::frame::pose::view;
use crate::domain::view::frame::screen::{ndc_to_px, size};

/// A fraction of the distance, since the marker holds a constant screen size.
/// Generous: one marker too many costs a sprite.
const ANCHOR_REACH: f64 = 0.08;

/// Indices into `candidates`, in the root frame.
pub fn select_visible(candidates: &mut Candidates, out: &mut Vec<u32>) {
    let v = view();
    let eye = v.at().eye;
    let [width, height] = size();

    let mut n = 0usize;
    for i in 0..candidates.cnt as usize {
        // Where the rays find it: the world's own point, nothing bent.
        let pos = [
            candidates.local_x[i],
            candidates.local_y[i],
            candidates.local_z[i],
        ];
        let to = v3::sub(&pos, &eye);
        let dist2 = v3::dot(&to, &to);
        if !v.frustum.sees(pos, dist2.sqrt() * ANCHOR_REACH) {
            continue;
        }
        candidates.d2[i] = dist2;
        candidates.order[n] = i as u32;
        n += 1;
    }

    // Nearest first; the tiebreak only stops equal distances swapping frame to frame.
    let at = |i: u32| candidates.d2[i as usize];
    let tiebreak = &candidates.tiebreak;
    candidates.order[..n].sort_by(|&a, &b| {
        at(a)
            .total_cmp(&at(b))
            .then(tiebreak[a as usize].total_cmp(&tiebreak[b as usize]))
    });

    out.clear();
    let mut ndc = [0.0; 3];

    for k in 0..n {
        let i = candidates.order[k] as usize;

        mat4::transform_point(
            &mut ndc,
            &v.world_view_proj,
            candidates.local_x[i],
            candidates.local_y[i],
            candidates.local_z[i],
        );
        let [sx, sy] = ndc_to_px([ndc[0], ndc[1]]);

        let w = candidates.region_px[i] as f64;
        let x0 = sx - w / 2.0;
        let y0 = sy - candidates.anchor_px[i] as f64;
        let region = [x0, y0, x0 + w, y0 + candidates.h_px[i] as f64];
        if region[2] < 0.0 || region[3] < 0.0 || x0 > width || y0 > height {
            continue;
        }
        candidates.screen[i] = region;
        out.push(i as u32);
    }
}
