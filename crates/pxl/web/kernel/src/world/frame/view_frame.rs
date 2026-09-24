// pxl-kernel/world/frame/view_frame.rs

//! The root frame's two damped halves (world/frame/view.rs): its turn
//! about Z toward the controls' azimuth, and its translation toward putting
//! the map centre at the origin. The floating origin is decided here, once a
//! tick, and rebased in the same step the translation is rewritten in it, so
//! nothing reads the two apart (coord.rs `local_world_origin`).

use pxl_spec::space::pvf3::fvec;
use pxl_spec::space::pvf3::ivec;
use xpute_core::math::scalar::wrap;

use crate::control::camera::config::camera_cfg::smooth::{POS_DAMP, ROT_DAMP};
use crate::control::camera::orbit::damp_alpha;
use crate::control::camera::store::camera_store;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::world::frame::coord::origin_for;
use crate::world::frame::view::{view, view_compose};
use crate::world::pyramid::window::window;

// see damping.rs's DIST_EPS/POLAR_EPS comment — the same reasoning about
// asking for frames.
const AZIMUTH_EPS: f64 = 0.0001;
const POS_EPS: f64 = 0.0005;

/// How far, in camera distances, the map centre may stand from where the view
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
    let v = view();
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

/// The "frame_translation" step of the interaction phase: toward the map
/// centre, in the floating origin taken from it.
pub fn view_translation_step(ctx: &mut TickContext) {
    let Some(map_center) = map_store().map_center else { return };
    let map_origin = origin_for(map_center.i, window().start);
    let s = steps();
    let v = view();

    let local_i = ivec::sub(map_center.i, map_origin);

    let tx = -(ivec::to_real(ivec::X(local_i)) + fvec::to_real(fvec::X(map_center.f)));
    let ty = -(ivec::to_real(ivec::Y(local_i)) + fvec::to_real(fvec::Y(map_center.f)));
    let tz = -(ivec::to_real(ivec::Z(local_i)) + fvec::to_real(fvec::Z(map_center.f)));

    // Where the view is, written against the origin decided now: the
    // damping carries on through a rebase rather than jumping to its target.
    if v.origin != map_origin {
        let d = ivec::sub(map_origin, v.origin);
        v.origin = map_origin;
        v.tx += ivec::to_real(ivec::X(d));
        v.ty += ivec::to_real(ivec::Y(d));
        v.tz += ivec::to_real(ivec::Z(d));
    }
    let gap = (tx - v.tx).hypot(ty - v.ty).hypot(tz - v.tz);
    if !s.translation_init || gap > camera_store().view_state.dist * JUMP_DISTANCES {
        s.translation_init = true;
        v.tx = tx;
        v.ty = ty;
        v.tz = tz;
        view_compose();
        return;
    }

    if tx == v.tx && ty == v.ty && tz == v.tz {
        return;
    }
    let alpha = 1.0 - (-POS_DAMP * ctx.delta).exp();
    v.tx += (tx - v.tx) * alpha;
    v.ty += (ty - v.ty) * alpha;
    v.tz += (tz - v.tz) * alpha;
    view_compose();

    if (tx - v.tx).abs() > POS_EPS || (ty - v.ty).abs() > POS_EPS || (tz - v.tz).abs() > POS_EPS {
        ctx.invalidate();
    }
}
