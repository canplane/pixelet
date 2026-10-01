// pxl-kernel/control/camera/store.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::math::scalar::clamp;

use crate::control::camera::config::camera_cfg::view::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MIN_DIST};
use crate::control::camera::controller::look_domain;
use crate::global::Global;
use crate::layer::object::store::EntryAt;
use crate::world::change::request_world_tick;

/// The controls held to what they range over: the distance to its bounds,
/// the look to its domain (orbit.rs `look_domain`).
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

/// What the controls ask of the camera: the look it is to turn to and the
/// orbit's distance it is to come to. Not the camera — that is the eye and
/// the look as drawn (controller.rs `Camera`), which eases toward this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intent {
    /// The orbit's distance, its radius.
    pub dist: f64,
    /// The look's heading about Z in radians, normalized to [-PI, PI].
    pub azimuth: f64,
    /// The look's angle down from vertical in radians.
    pub polar: f64,
}

/// A partial intent: what a setter was handed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IntentPatch {
    pub dist: Option<f64>,
    pub azimuth: Option<f64>,
    pub polar: Option<f64>,
}

pub struct CameraStore {
    pub camera_status: CameraStatus,
    pub hovered_coord: Option<Pvf3>,
    /// Where the placed object currently under the cursor is filed, if there
    /// is one — see camera/ray.rs's PickResult.
    pub hovered_object_at: Option<EntryAt>,
    /// What the controls ask of the camera (`Intent`).
    pub intent: Intent,
    /// Follow mode flag.
    pub is_follow_mode: bool,
}

impl CameraStore {
    fn new() -> CameraStore {
        CameraStore {
            camera_status: CameraStatus::Idle,
            hovered_coord: None,
            hovered_object_at: None,
            intent: Intent {
                dist: DEFAULT_DIST,
                azimuth: DEFAULT_AZIMUTH,
                polar: DEFAULT_POLAR,
            },
            is_follow_mode: false,
        }
    }

    /// `partial` over the intent as it stands, each held to what it ranges
    /// over: what a control asks, before the active controller has its say
    /// (controller.rs `set_intent`).
    pub fn merged(&self, partial: IntentPatch) -> Intent {
        let from = self.intent;
        Intent {
            dist: norm_dist(partial.dist.unwrap_or(from.dist)),
            azimuth: norm_azimuth(partial.azimuth.unwrap_or(from.azimuth)),
            polar: norm_polar(partial.polar.unwrap_or(from.polar)),
        }
    }

    /// A view state change wakes the damping loop, which only runs when a
    /// frame is asked for.
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
