// pxl-core/domain/scene/marker/selection.rs

//! Which world-anchored markers are visible. A marker is visible when a
//! sphere about its anchor is in the frustum and its box is on the screen.
//! The visible markers are listed nearest first.

use pxl_spec::space::coord::mat4;
use pxl_spec::space::coord::v3;

use crate::domain::scene::marker::candidate::Candidates;
use crate::domain::scene::marker::param::ANCHOR_RADIUS;
use crate::domain::view::frame::pose::view;
use crate::domain::view::frame::screen::{ndc_to_px, size};

/// Writes into `out` the indices into `candidates` of the visible markers,
/// nearest first. The candidates are in the root frame.
pub(super) fn select_visible(candidates: &mut Candidates, out: &mut Vec<u32>) {
    let v = view();
    let eye = v.at().eye;
    let [width, height] = size();

    let mut n = 0usize;
    for i in 0..candidates.cnt as usize {
        // The anchor is tested where the rays find it: at the world's own
        // point, with nothing bent.
        let pos = [
            candidates.local_x[i],
            candidates.local_y[i],
            candidates.local_z[i],
        ];
        let to = v3::sub(&pos, &eye);
        let dist2 = v3::dot(&to, &to);
        if !v.frustum.sees(pos, dist2.sqrt() * ANCHOR_RADIUS) {
            continue;
        }
        candidates.d2[i] = dist2;
        candidates.order[n] = i as u32;
        n += 1;
    }

    // Nearest first. The tiebreak only keeps markers at equal distances from
    // swapping from frame to frame.
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
