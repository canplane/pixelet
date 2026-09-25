// pxl-kernel/control/camera/damping.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::math::scalar::clamp;

use crate::control::camera::config::camera_cfg::smooth::CAM_DAMP;
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MAX_POLAR, MIN_DIST, MIN_POLAR};
use crate::control::camera::orbit::damp_alpha;
use crate::control::camera::store::camera_store;
use crate::control::camera::terrain_gate::climb_polar;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, CameraTerrainMode};
use crate::world::frame::view::{view, view_compose};
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;

// The tick runs another frame only when something asks (sched/tick.rs) —
// this damping loop is the main thing asking while the camera is settling
// after a drag/zoom, and needs to stop once it's close enough that a further
// frame wouldn't look any different, or it would ask forever and the view
// would never be idle. World units for dist, radians for polar — both small
// enough that the remaining error is sub-pixel at any zoom level in view.
const DIST_EPS: f64 = 0.001;
const POLAR_EPS: f64 = 0.0001;

/// How quickly the climb's own contribution follows what terrain asks for.
///
/// Elevation is read per cell, so the answer steps rather than slides. Rising
/// is the faster of the two: late to rise means the eye is briefly inside a
/// hill, late to relax means the view sits steeper than it needs to for a
/// moment longer.
const CLIMB_RISE_DAMP: f64 = 20.0;
const CLIMB_RELAX_DAMP: f64 = 5.0;
const CLIMB_EPS: f64 = 0.0005;

/// climb_polar's answer for the last inputs: the climb samples the terrain
/// twice through the page tables and page math, and every ticked frame asked
/// it again with the view at rest. The answer changes only with the view, the
/// map centre, or a page arriving — the terrain pyramid's revision moves for
/// the last, which is also when ground not known yet becomes known.
struct ClimbMemo {
    dist: f64,
    polar: f64,
    azimuth: f64,
    centre: Option<Pvf3>,
    revision: u32,
    polar_out: f64,
}

struct CameraStep {
    did_init: bool,
    dist: f64,
    polar: f64,
    /// How much steeper than the control the drawn pitch currently is, always
    /// <= 0. Zero unless terrain is being climbed over; view_state.polar is
    /// never touched by it.
    shown_climb: f64,
    memo: ClimbMemo,
}

static STEP: Global<CameraStep> = Global::new();

fn step_state() -> &'static mut CameraStep {
    STEP.get(|| CameraStep {
        did_init: false,
        dist: 0.0,
        polar: 0.0,
        shown_climb: 0.0,
        memo: ClimbMemo {
            dist: f64::NAN,
            polar: f64::NAN,
            azimuth: f64::NAN,
            centre: None,
            revision: u32::MAX,
            polar_out: f64::NAN,
        },
    })
}

fn climb_polar_memo(memo: &mut ClimbMemo, dist: f64, polar: f64, azimuth: f64) -> f64 {
    let centre = map_store().map_center;
    let revision = Terrain::pyramid().revision();
    if dist == memo.dist && polar == memo.polar && azimuth == memo.azimuth && centre == memo.centre && revision == memo.revision {
        return memo.polar_out;
    }
    let out = climb_polar(dist, polar, azimuth);
    *memo = ClimbMemo {
        dist,
        polar,
        azimuth,
        centre,
        revision,
        polar_out: out,
    };
    out
}

/// Damps the eye's distance and pitch toward the controls and writes them
/// into the view (world/frame/view.rs): the "camera" step of the
/// interaction phase.
pub fn camera_step(ctx: &mut TickContext) {
    let view_state = camera_store().view_state;
    let s = step_state();

    if !s.did_init {
        s.dist = view_state.dist;
        s.polar = view_state.polar;
        s.shown_climb = 0.0;
        s.did_init = true;
    } else {
        let alpha = damp_alpha(CAM_DAMP, ctx.delta);
        s.dist += (view_state.dist - s.dist) * alpha;
        s.polar += (view_state.polar - s.polar) * alpha;
    }

    s.dist = clamp(s.dist, MIN_DIST, MAX_DIST);
    s.polar = clamp(s.polar, MIN_POLAR, MAX_POLAR);

    // still settling — ask for one more frame. See DIST_EPS/POLAR_EPS above.
    if (view_state.dist - s.dist).abs() > DIST_EPS || (view_state.polar - s.polar).abs() > POLAR_EPS {
        ctx.invalidate();
    }

    // `climb` grants the control and answers terrain by pitching the view up
    // instead. dist stays exactly what the scale bar reads; only the angle
    // gives way, and it comes back on its own once the ground does. What is
    // smoothed here is the climb's own contribution, never the pitch itself:
    // polar is already damped alongside dist just above, and running the
    // absolute angle through a second lag gave tilt an inertia rotation does
    // not have. Outside climb mode the offset is zero and the drawn pitch is
    // exactly the damped control.
    let want_climb = if settings_store().camera_terrain == CameraTerrainMode::climb {
        climb_polar_memo(&mut s.memo, s.dist, s.polar, view_state.azimuth) - s.polar
    } else {
        0.0
    };
    let climb_k = if want_climb < s.shown_climb { CLIMB_RISE_DAMP } else { CLIMB_RELAX_DAMP };
    s.shown_climb += (want_climb - s.shown_climb) * damp_alpha(climb_k, ctx.delta);
    if (want_climb - s.shown_climb).abs() > CLIMB_EPS {
        ctx.invalidate();
    }

    // The offset lags the control, so a pitch steepening toward vertical can
    // still be carrying a climb from a shallower one.
    let view_polar = clamp(s.polar + s.shown_climb, MIN_POLAR, MAX_POLAR);
    let v = view();
    if v.dist != s.dist || v.polar != view_polar {
        v.dist = s.dist;
        v.polar = view_polar;
        view_compose();
    }
}
