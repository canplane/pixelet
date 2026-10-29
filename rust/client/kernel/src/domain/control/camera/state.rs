// pxl-kernel/domain/control/camera/state.rs

use xpute_kit::math::scalar::clamp;

use crate::domain::control::camera::controller::facing_domain;
use crate::domain::control::param::{
    DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MIN_DIST,
};
use crate::sys::sched::edge::request_world_tick;
use xpute_guest::global::Global;

fn norm_dist(dist: f64) -> f64 {
    clamp(dist, MIN_DIST, MAX_DIST)
}

fn norm_azimuth(azimuth: f64) -> f64 {
    facing_domain(0.0, azimuth).1
}

fn norm_polar(polar: f64) -> f64 {
    facing_domain(polar, 0.0).0
}

/// What the controls ask; the drawn camera (controller.rs `Camera`) eases toward it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intent {
    pub dist: f64,
    /// Heading about Z in radians, in [-PI, PI].
    pub azimuth: f64,
    /// Angle down from vertical in radians.
    pub polar: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntentPatch {
    pub dist: Option<f64>,
    pub azimuth: Option<f64>,
    pub polar: Option<f64>,
}

pub struct CameraState {
    pub intent: Intent,
}

impl CameraState {
    fn new() -> CameraState {
        CameraState {
            intent: Intent {
                dist: DEFAULT_DIST,
                azimuth: DEFAULT_AZIMUTH,
                polar: DEFAULT_POLAR,
            },
        }
    }

    /// Before the active controller has its say (controller.rs `set_intent`).
    pub fn merged(&self, partial: IntentPatch) -> Intent {
        let from = self.intent;
        Intent {
            dist: norm_dist(partial.dist.unwrap_or(from.dist)),
            azimuth: norm_azimuth(partial.azimuth.unwrap_or(from.azimuth)),
            polar: norm_polar(partial.polar.unwrap_or(from.polar)),
        }
    }

    /// Wakes the damping loop, which runs only when a frame is asked for.
    pub(crate) fn commit_intent(&mut self, next: Intent) {
        if self.intent == next {
            return;
        }
        self.intent = next;
        request_world_tick();
    }
}

static CAMERA_STATE: Global<CameraState> = Global::new();

pub fn camera_state() -> &'static mut CameraState {
    CAMERA_STATE.get(CameraState::new)
}
