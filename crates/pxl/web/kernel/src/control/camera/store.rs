// pxl-kernel/control/camera/store.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::math::scalar::clamp;

use crate::control::camera::config::camera_cfg::view::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MIN_DIST};
use crate::control::camera::controller::look_domain;
use crate::global::Global;
use crate::layer::decor::hit::Scattered;
use crate::layer::object::store::EntryAt;
use crate::world::change::request_world_tick;
use xpute_runtime::global::Global;

fn norm_dist(dist: f64) -> f64 {
    clamp(dist, MIN_DIST, MAX_DIST)
}

fn norm_azimuth(azimuth: f64) -> f64 {
    look_domain(0.0, azimuth).1
}

fn norm_polar(polar: f64) -> f64 {
    look_domain(polar, 0.0).0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraStatus {
    #[default]
    Idle,
    Drag,
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

pub struct CameraStore {
    pub camera_status: CameraStatus,
    pub hovered_coord: Option<Pvf3>,
    pub hovered_object_at: Option<EntryAt>,
    /// The scattered decor under the cursor, if that is what is under it:
    /// no entry, what is made there.
    pub hovered_scattered: Option<Scattered>,
    /// What the controls ask of the camera (`Intent`).
    pub intent: Intent,
    pub is_follow_mode: bool,
}

impl CameraStore {
    fn new() -> CameraStore {
        CameraStore {
            camera_status: CameraStatus::Idle,
            hovered_coord: None,
            hovered_object_at: None,
            hovered_scattered: None,
            intent: Intent {
                dist: DEFAULT_DIST,
                azimuth: DEFAULT_AZIMUTH,
                polar: DEFAULT_POLAR,
            },
            travel: [0.0; 2],
            is_follow_mode: false,
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

static CAMERA_STORE: Global<CameraStore> = Global::new();

pub fn camera_store() -> &'static mut CameraStore {
    CAMERA_STORE.get(CameraStore::new)
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
