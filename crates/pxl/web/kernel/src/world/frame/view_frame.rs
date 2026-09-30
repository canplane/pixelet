// pxl-kernel/world/frame/view_frame.rs

//! The orbit's heading and pivot (camera/orbit.rs), each damped toward its
//! control: the heading toward the controls' azimuth, the pivot toward the
//! map center. The floating origin is decided here, and rebased in the same
//! step the pivot is rewritten in it, so nothing reads the two apart
//! (coord.rs `local_world_origin`). A jump of the map center is no
//! damping's: it is put in place before the turn observes, and again by the
//! tick where the tick's own input made it.

use pxl_spec::space::pvf3::fvec;
use pxl_spec::space::pvf3::ivec;
use xpute_core::math::scalar::wrap;

use crate::control::camera::config::camera_cfg::smooth::{POS_DAMP, ROT_DAMP};
use crate::control::camera::orbit::{damp_alpha, orbit_view};
use crate::control::camera::store::camera_store;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::origin_for;
use crate::world::frame::view::{view, view_compose};
use crate::world::observation::ground_level;
use pxl_spec::space::v3::V3;

// see damping.rs's DIST_EPS/POLAR_EPS comment — the same reasoning about
// asking for frames.
const AZIMUTH_EPS: f64 = 0.0001;
const POS_EPS: f64 = 0.0005;

/// How far, in camera distances, the map center may stand from where the view
/// is for the view to glide there; past it the move is a jump — a search, a
/// locate — and the view is put there at once. A drag's lag is the damping's
/// tenth of a second of travel, a fraction of a screen, and a screen is about
/// a camera distance across.
const JUMP_DISTANCES: f64 = 1.0;

struct FrameSteps {
    rotation_init: bool,
    translation_init: bool,
}

static STEPS: Global<FrameSteps> = Global::new();

fn steps() -> &'static mut FrameSteps {
    STEPS.get(|| FrameSteps {
        rotation_init: false,
        translation_init: false,
    })
}

/// The "frame_rotation" step of the interaction phase.
pub fn view_rotation_step(ctx: &mut TickContext) {
    let azimuth = camera_store().view_state.azimuth;
    let s = steps();
    let v = orbit_view();
    if !s.rotation_init {
        v.azimuth = azimuth;
        view_compose();
        s.rotation_init = true;
        return;
    }

    let alpha = damp_alpha(ROT_DAMP, ctx.delta);
    let err = wrap(azimuth - v.azimuth, -core::f64::consts::PI, core::f64::consts::PI);
    if err == 0.0 {
        return;
    }
    v.azimuth += err * alpha;
    view_compose();

    if (err * (1.0 - alpha)).abs() > AZIMUTH_EPS {
        ctx.invalidate();
    }
}

/// Where the view is to stand for the map center, written against the
/// floating origin taken from it, the view rebased onto that origin in the
/// same step — the damping carries on through a rebase rather than jumping
/// to its target; and the view put there at once where it is not placed yet
/// or the map center is a jump away — a search, a locate — which is what
/// the second of the pair says. None before there is a map center.
fn snap_if_discontinuous() -> Option<(V3, bool)> {
    let map_center = map_store().map_center?;
    let map_origin = origin_for(map_center.i, ground_level());
    let s = steps();
    let v = view();
    let o = orbit_view();

    let local_i = ivec::sub(map_center.i, map_origin);
    let target = [
        ivec::to_real(ivec::X(local_i)) + fvec::to_real(fvec::X(map_center.f)),
        ivec::to_real(ivec::Y(local_i)) + fvec::to_real(fvec::Y(map_center.f)),
        ivec::to_real(ivec::Z(local_i)) + fvec::to_real(fvec::Z(map_center.f)),
    ];
    if v.origin != map_origin {
        let d = ivec::sub(map_origin, v.origin);
        v.origin = map_origin;
        o.pivot[0] -= ivec::to_real(ivec::X(d));
        o.pivot[1] -= ivec::to_real(ivec::Y(d));
        o.pivot[2] -= ivec::to_real(ivec::Z(d));
    }
    let gap = (target[0] - o.pivot[0]).hypot(target[1] - o.pivot[1]).hypot(target[2] - o.pivot[2]);
    if s.translation_init && gap <= camera_store().view_state.dist * JUMP_DISTANCES {
        return Some((target, false));
    }
    s.translation_init = true;
    o.pivot = target;
    view_compose();
    Some((target, true))
}

/// The view put where the map center is before anything observes it, where
/// that is a jump away or the view is not placed yet: what the effects turn
/// set the map center to is what the turn's demand is made from, and a
/// search asks for the pages of where it went and not of where the view
/// was. A move within the jump distance is left to the tick's damping, so
/// what is observed is the view being drawn and not the control's target.
pub fn view_snap_step() {
    snap_if_discontinuous();
}

/// The "frame_translation" step of the interaction phase: toward the map
/// center, in the floating origin taken from it. A jump the tick's own input
/// made is put there at once too, and asks for the turn after, whose effects
/// observe it.
pub fn view_translation_step(ctx: &mut TickContext) {
    let Some((target, snapped)) = snap_if_discontinuous() else { return };
    if snapped {
        ctx.invalidate();
        return;
    }
    let p = &mut orbit_view().pivot;
    if *p == target {
        return;
    }
    let alpha = 1.0 - (-POS_DAMP * ctx.delta).exp();
    for k in 0..3 {
        p[k] += (target[k] - p[k]) * alpha;
    }
    let settled = (0..3).all(|k| (target[k] - p[k]).abs() <= POS_EPS);
    view_compose();
    if !settled {
        ctx.invalidate();
    }
}

#[cfg(test)]
#[path = "view_frame.test.rs"]
mod test;
