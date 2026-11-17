// pxl-core/domain/control/camera/state.rs

use crate::spec::step::InvalidationReason;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::clamp;

use crate::domain::control::camera::facing_domain;
use crate::domain::control::param::{
    DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MIN_DIST,
};
use crate::domain::control::support::resolve_to;
use crate::domain::param::POS_DAMP;
use crate::domain::smooth::smooth_vec;
use crate::sys::sched::invalidate;
use pxl_global::Global;

/// The facing and the distance the controls ask the camera for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Intent {
    pub dist: f64,
    /// Radians about Z from north toward east, in [-PI, PI].
    pub azimuth: f64,
    /// Radians from straight down, in [0, PI].
    pub polar: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct IntentPatch {
    pub dist: Option<f64>,
    pub azimuth: Option<f64>,
    pub polar: Option<f64>,
}

/// What the controls ask the camera for, which the drawn camera (camera.rs
/// `Camera`) eases toward: the intent, and where the pivot goes, `to`, a
/// canonical point so that it stays the same when the eye goes round the
/// world. `to` is None until it is first put (controller.rs `put_pivot`), and
/// whichever controller moves the camera eases its pivot toward it
/// (`ease_pivot`).
pub(crate) struct CameraState {
    pub intent: Intent,
    pub to: Option<Q3>,
}

impl CameraState {
    fn new() -> CameraState {
        CameraState {
            intent: Intent {
                dist: DEFAULT_DIST,
                azimuth: DEFAULT_AZIMUTH,
                polar: DEFAULT_POLAR,
            },
            to: None,
        }
    }

    /// The intent with `partial` applied and held to the camera's ranges.
    /// The orbit may still change it before it is committed (controller.rs
    /// `set_intent`).
    pub(crate) fn with_patch(&self, partial: IntentPatch) -> Intent {
        let from = self.intent;
        let (polar, azimuth) = facing_domain(
            partial.polar.unwrap_or(from.polar),
            partial.azimuth.unwrap_or(from.azimuth),
        );
        Intent {
            dist: clamp(partial.dist.unwrap_or(from.dist), MIN_DIST, MAX_DIST),
            azimuth,
            polar,
        }
    }

    /// The camera eases toward its intent only on a frame that was asked
    /// for, so a changed intent asks for a frame.
    pub(crate) fn commit_intent(&mut self, next: Intent) {
        if self.intent == next {
            return;
        }
        self.intent = next;
        invalidate(InvalidationReason::View);
    }
}

static CAMERA_STATE: Global<CameraState> = Global::new();

pub(crate) fn camera_state() -> &'static CameraState {
    camera_state_mut()
}

pub(crate) fn camera_state_mut() -> &'static mut CameraState {
    CAMERA_STATE.get(CameraState::new)
}

/// The pivot's `to` as a point in the world, for a pivot drawn at `cur`
/// (control/support.rs `resolve_to`); `cur` itself before a `to` is put.
pub(crate) fn resolved_to(cur: V3) -> V3 {
    resolve_to(&mut camera_state_mut().to, cur)
}

/// Eases the pivot drawn at `cur` toward `to` over `delta` seconds, and
/// returns where it is drawn now.
pub(crate) fn ease_pivot(cur: V3, to: V3, delta: f64) -> V3 {
    smooth_vec(cur, to, POS_DAMP, delta)
}
