// pxl-kernel/control/camera/store.rs

use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::math::scalar::{clamp, round_to_step, wrap};

use crate::control::camera::config::camera_cfg::view::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR, MAX_DIST, MAX_POLAR, MIN_DIST, MIN_POLAR};
use crate::control::camera::terrain_gate::{allowed_view, ViewTriple};
use crate::global::Global;
use crate::layer::object::store::EntryAt;
use crate::world::change::request_world_tick;

const PI: f64 = core::f64::consts::PI;

fn norm_dist(dist: f64) -> f64 {
    clamp(round_to_step(dist, 0.1), MIN_DIST, MAX_DIST)
}

fn norm_azimuth(azimuth: f64) -> f64 {
    wrap(round_to_step(azimuth, 0.1 * (PI / 180.0)), -PI, PI)
}

fn norm_polar(polar: f64) -> f64 {
    clamp(round_to_step(polar, 0.01), MIN_POLAR, MAX_POLAR)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraStatus {
    #[default]
    Idle,
    Drag,
}

/// Camera view intent.
///
/// This is not the raw physical camera transform. It stores the
/// user/system-facing view parameters used to derive a pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewState {
    /// Camera-to-target distance.
    pub dist: f64,
    /// Orbit azimuth in radians, normalized to [-PI, PI].
    pub azimuth: f64,
    /// Orbit polar angle in radians.
    pub polar: f64,
}

impl ViewState {
    fn triple(self) -> ViewTriple {
        ViewTriple {
            dist: self.dist,
            polar: self.polar,
            azimuth: self.azimuth,
        }
    }
}

/// A partial view state: what a setter was handed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewPatch {
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
    /// User/system-facing camera view intent.
    pub view_state: ViewState,
    /// Follow mode flag.
    pub is_follow_mode: bool,
    /// A wheel's zoom is still reaching its distance: the wheel has no hold to
    /// let go of, so its gesture ends when the eye arrives (damping.rs).
    pub wheel_zooming: bool,
}

impl CameraStore {
    fn new() -> CameraStore {
        CameraStore {
            camera_status: CameraStatus::Idle,
            hovered_coord: None,
            hovered_object_at: None,
            view_state: ViewState {
                dist: DEFAULT_DIST,
                azimuth: DEFAULT_AZIMUTH,
                polar: DEFAULT_POLAR,
            },
            is_follow_mode: false,
            wheel_zooming: false,
        }
    }

    pub fn set_view_state(&mut self, partial: ViewPatch) {
        let from = self.view_state;
        let mut next = ViewState {
            dist: norm_dist(partial.dist.unwrap_or(from.dist)),
            azimuth: norm_azimuth(partial.azimuth.unwrap_or(from.azimuth)),
            polar: norm_polar(partial.polar.unwrap_or(from.polar)),
        };
        // As far as the control asked, or as far as terrain allows.
        let reached = allowed_view(next.triple(), from.triple());
        next.dist = norm_dist(reached.dist);
        next.azimuth = norm_azimuth(reached.azimuth);
        next.polar = norm_polar(reached.polar);
        self.commit_view(next);
    }

    pub fn reset_dist(&mut self) {
        let dist = norm_dist(DEFAULT_DIST);
        let from = self.view_state;
        if from.dist == dist {
            return;
        }
        let reached = allowed_view(ViewState { dist, ..from }.triple(), from.triple());
        self.commit_view(ViewState {
            dist: norm_dist(reached.dist),
            ..from
        });
    }

    // azimuth-only: "align north" is what both the dedicated compass button
    // and the follow-mode-off action actually mean, so polar (tilt) is left
    // as the user set it.
    pub fn reset_azimuth(&mut self) {
        let azimuth = norm_azimuth(DEFAULT_AZIMUTH);
        let from = self.view_state;
        if from.azimuth == azimuth {
            return;
        }
        let reached = allowed_view(ViewState { azimuth, ..from }.triple(), from.triple());
        self.commit_view(ViewState {
            azimuth: norm_azimuth(reached.azimuth),
            ..from
        });
    }

    /// A view state change wakes the damping loop, which only runs when a
    /// frame is asked for.
    fn commit_view(&mut self, next: ViewState) {
        if self.view_state == next {
            return;
        }
        self.view_state = next;
        request_world_tick();
    }
}

static CAMERA_STORE: Global<CameraStore> = Global::new();

pub fn camera_store() -> &'static mut CameraStore {
    CAMERA_STORE.get(CameraStore::new)
}
