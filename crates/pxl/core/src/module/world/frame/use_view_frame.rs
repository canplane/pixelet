// pxl_core/module/world/frame/use_view_frame.rs

//! The root frame's two damped halves (module/world/frame/view.rs): its turn
//! about Z toward the controls' azimuth, and its translation toward putting
//! the map centre at the origin — the view's origin rebased in the same step
//! the translation snaps, so nothing reads the two apart for a tick.

use pxl_shared::spec::space::pvf3::fvec;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use xpute_core::kit::math::wrap;

use super::coord::get_local_world_origin;
use super::view::{view, view_compose};
use crate::global::Global;
use crate::module::camera::config::camera_cfg::smooth::{POS_DAMP, ROT_DAMP};
use crate::module::camera::orbit::damp_alpha;
use crate::module::camera::store::use_camera_store;
use crate::module::map::store::use_map_store;
use crate::module::world::sched::tick::TickContext;

// see use_camera.rs's DIST_EPS/POLAR_EPS comment — the same reasoning about
// asking for frames.
const AZIMUTH_EPS: f64 = 0.0001;
const POS_EPS: f64 = 0.0005;

struct FrameSteps {
    rotation_init: bool,
    translation_init: bool,
    prev_map_origin: Option<IVec>,
}

static STEPS: Global<FrameSteps> = Global::new();

fn steps() -> &'static mut FrameSteps {
    STEPS.get(|| FrameSteps {
        rotation_init: false,
        translation_init: false,
        prev_map_origin: None,
    })
}

/// The "frame_rotation" step of the interaction phase.
pub fn view_rotation_step(ctx: &mut TickContext) {
    let azimuth = use_camera_store().view_state.azimuth;
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
        ctx.view_moving();
    }
}

/// The "frame_translation" step of the interaction phase: toward the map
/// centre, in the floating origin taken from it.
pub fn view_translation_step(ctx: &mut TickContext) {
    let Some(map_center) = use_map_store().map_center else { return };
    let map_origin = get_local_world_origin(Some(&map_center));
    let s = steps();
    let v = view();

    let did_rebase = s.prev_map_origin != Some(map_origin);
    s.prev_map_origin = Some(map_origin);

    if did_rebase {
        v.origin = map_origin;
    }

    let local_i = ivec::sub(map_center.i, map_origin);

    let tx = -(ivec::to_real(ivec::X(local_i)) + fvec::to_real(fvec::X(map_center.f)));
    let ty = -(ivec::to_real(ivec::Y(local_i)) + fvec::to_real(fvec::Y(map_center.f)));
    let tz = -(ivec::to_real(ivec::Z(local_i)) + fvec::to_real(fvec::Z(map_center.f)));

    if !s.translation_init || did_rebase {
        s.translation_init = true;
        v.origin = map_origin;
        v.tx = tx;
        v.ty = ty;
        v.tz = tz;
        view_compose();
        ctx.view_moving();
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
        ctx.view_moving();
    }
}
