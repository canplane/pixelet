// pxl-kernel/scene/environment.rs

//! Time of day and the light it gives: where the world is anchored and the
//! wall clock as inputs; the sun and moon placed, and the light and sky they
//! give.

pub mod celestial_position;
pub mod cloud;

use pxl_render::look::environment::{write_environment, Light, ENV_FLOATS};
use pxl_spec::space::coord::v3::len3;

use crate::chunk::table::Frame;
use crate::io::host::wall_now;
use crate::kernel;
use crate::sched::edge::wake_at;
use crate::view::frame::pose::{anchor, globe, km_per_u};
use crate::view::reach::eye_ground;
use celestial_position::{equatorial_to_local, j2000_days, moon_position, sun_position};
use xpute_guest::clock::now;
use xpute_guest::global::Global;

// The sky moves with the wall clock and nothing else asks for a frame while
// the view is still, so the clock asks for one this often.
const ENV_CLOCK_FRAME_MS: f64 = 2000.0;

struct Environment {
    clock_frame_at: f64,
}

static ENVIRONMENT: Global<Environment> = Global::new();

fn environment() -> &'static mut Environment {
    ENVIRONMENT.get(|| Environment { clock_frame_at: 0.0 })
}

/// Runs before the chunks' step.
pub fn environment_step() {
    let e = environment();
    let sched = &mut kernel().scene;

    let unix_ms = wall_now();
    sched.environment.observe(unix_ms);

    if unix_ms - e.clock_frame_at >= ENV_CLOCK_FRAME_MS {
        e.clock_frame_at = unix_ms;
    }
    wake_at(now() + e.clock_frame_at + ENV_CLOCK_FRAME_MS - unix_ms);
}

/// Below this the transition counts as done and stops asking for frames.
const SETTLE_EPS: f64 = 0.005;
/// The sun and moon follow a jump of the anchor in about 1.5 s and never step.
const EASE_RATE: f64 = 1.5;

pub struct EnvironmentDraw {
    started: bool,
    /// Days since J2000.0, from the host's wall clock.
    days: f64,
    sun: [f64; 3],
    moon: [f64; 3],
    stars: f64,
    /// Seconds of wall time since the first observation: a tick's delta is
    /// clamped and sparse at rest, which ran clouds and stars slow, and the
    /// epoch would not survive the shader's f32.
    time: f64,
    first_ms: Option<f64>,
    pub out: [f32; ENV_FLOATS],
}

impl Default for EnvironmentDraw {
    fn default() -> Self {
        EnvironmentDraw {
            started: false,
            days: 0.0,
            sun: [0.0; 3],
            moon: [0.0; 3],
            stars: 0.0,
            time: 0.0,
            first_ms: None,
            out: [0.0; ENV_FLOATS],
        }
    }
}

impl EnvironmentDraw {
    /// The sun under the horizon, as drawn.
    pub fn night(&self) -> bool {
        self.started && self.sun[2] < 0.0
    }

    /// Unix milliseconds.
    pub fn observe(&mut self, unix_ms: f64) {
        self.days = j2000_days(unix_ms);
        let first = *self.first_ms.get_or_insert(unix_ms);
        self.time = (unix_ms - first) / 1000.0;
    }

    /// Whether a transition is still in flight and wants frames.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        if self.first_ms.is_none() {
            return false;
        }
        // Observed from the anchor, which the world is laid on the planet about.
        let [lon, lat, _] = anchor();
        let sun = sun_position(self.days, lat, lon);
        let moon = moon_position(self.days, lat, lon);
        let a = if self.started { (frame.delta_s * EASE_RATE).clamp(0.0, 1.0) } else { 1.0 };
        if !self.started {
            self.sun = sun;
            self.moon = moon;
            self.started = true;
        }
        for k in 0..3 {
            self.sun[k] += (sun[k] - self.sun[k]) * a;
            self.moon[k] += (moon[k] - self.moon[k]) * a;
        }
        // The stars start dark and fade in, the first frame included.
        let star_a = (frame.delta_s * EASE_RATE).clamp(0.0, 1.0);
        self.stars += (1.0 - self.stars) * star_a;
        // Height over the planet: the world is tangent to it at the camera center
        // and the planet falls away under a camera standing off it.
        let km_per_u = km_per_u();
        let r = globe().radius_u();
        let over = len3(&[frame.camera[0], frame.camera[1], frame.camera[2] + r]) - r;
        let light = Light {
            sun: self.sun,
            moon: self.moon,
            stars: self.stars,
            time: self.time,
            altitude_km: over * km_per_u,
            ground_km: eye_ground() * km_per_u,
            km_per_u,
            latitude: lat.to_radians(),
            longitude: lon.to_radians(),
            star_frame: equatorial_to_local(self.days, lat, lon),
        };
        write_environment(&mut self.out, &light);
        1.0 - self.stars > SETTLE_EPS
    }
}
