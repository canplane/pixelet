// pxl-kernel/scene/environment.rs

//! Time of day and the light it gives: where the world is anchored and the
//! wall clock as inputs; the sun and moon placed, and the light and sky they
//! give.

pub mod celestial_position;
pub mod cloud;

use pxl_render::gpu::renderer::sky_steps;
use pxl_render::look::environment::{write_environment, Light, ENV_FLOATS};
use pxl_spec::space::coord::v3;

use crate::io::host::wall_now;
use crate::kernel;
use crate::scene::frame::Frame;
use crate::sched::edge::wake_at;
use crate::view::frame::pose::{anchor, globe, km_per_u};
use crate::view::reach::eye_ground;
use celestial_position::{equatorial_to_local, j2000_days, moon_position, sun_position};
use xpute_guest::clock::now;

/// Runs before the chunks' step: the sky moves with the wall clock and
/// nothing else asks for a frame while the view is still, so the clock asks
/// for one when its next step falls (`EnvironmentDraw::observe`).
pub fn environment_step() {
    let next_ms = kernel().scene.environment.observe(wall_now());
    wake_at(now() + next_ms);
}

/// The sun and moon follow a jump of the anchor, and the stars fade in, over
/// 1/EASE_RATE s, and never step.
const EASE_RATE: f64 = 1.5;

pub struct EnvironmentDraw {
    started: bool,
    /// Days since J2000.0 the sky is drawn for, from the host's wall clock
    /// a step at a time (`observe`).
    days: f64,
    sun: [f64; 3],
    moon: [f64; 3],
    /// The anchor the sky is observed from, where the sun and moon stood when
    /// it last jumped, and how far they have come since, to one at the end.
    anchor: [f64; 2],
    from: [[f64; 3]; 2],
    eased: f64,
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
            anchor: [0.0; 2],
            from: [[0.0; 3]; 2],
            eased: 1.0,
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

    /// Unix milliseconds: the time, and the sky's days a step at a time, a
    /// step where the sun's or the moon's height over the anchor has moved
    /// its step since the last, the height over which the sky as drawn moves
    /// one display level (pxl-render `sky_steps`, its views' own slopes) — so
    /// the sky's words stay the same between steps, and its tables are drawn
    /// again by the same measure; and how many milliseconds on the next step
    /// falls, as their heights move now. A light with no step landed yet
    /// asks none: its view's first slope landing wakes the turn.
    pub fn observe(&mut self, unix_ms: f64) -> f64 {
        let first = *self.first_ms.get_or_insert(unix_ms);
        self.time = (unix_ms - first) / 1000.0;
        let [lon, lat, _] = anchor();
        // The cosine of each one's zenith angle: its direction's up, the
        // positions standing at a distance.
        let mu = |p: [f64; 3]| p[2] / v3::len(&p);
        let heights = |days: f64| [mu(sun_position(days, lat, lon)), mu(moon_position(days, lat, lon))];
        let (step, days) = (sky_steps().map(|s| s.map_or(f64::INFINITY, f64::from)), j2000_days(unix_ms));
        let moved = |from: [f64; 2], to: [f64; 2]| [0, 1].map(|k| (to[k] - from[k]).abs());
        if !self.started || moved(heights(self.days), heights(days)).iter().zip(step).any(|(&d, step)| d >= step) {
            self.days = days;
        }
        // As fast as each height moves over the next minute, when it will have moved a step.
        const MINUTE_MS: f64 = 60_000.0;
        let (now, drawn) = (heights(days), heights(self.days));
        let rate = moved(now, heights(days + MINUTE_MS / 86_400_000.0));
        let gone = moved(drawn, now);
        (0..2).map(|k| (step[k] - gone[k]).max(0.0) * MINUTE_MS / rate[k]).fold(f64::INFINITY, f64::min)
    }

    /// Whether a transition is still in flight and wants frames: the sun
    /// and moon on their way across a jump of the anchor, the stars fading
    /// in. The clock's steps are taken as they come (`observe`), a display
    /// level each, so a still sky's words stand between them.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        if self.first_ms.is_none() {
            return false;
        }
        // Observed from the anchor, which the world is laid on the planet about.
        let [lon, lat, _] = anchor();
        let sun = sun_position(self.days, lat, lon);
        let moon = moon_position(self.days, lat, lon);
        if !self.started {
            (self.sun, self.moon, self.anchor) = (sun, moon, [lon, lat]);
            self.started = true;
        }
        if self.anchor != [lon, lat] {
            (self.from, self.eased, self.anchor) = ([self.sun, self.moon], 0.0, [lon, lat]);
        }
        self.eased = (self.eased + frame.delta_s * EASE_RATE).min(1.0);
        let along = |from: [f64; 3], to: [f64; 3]| {
            if self.eased < 1.0 {
                core::array::from_fn(|k| from[k] + (to[k] - from[k]) * self.eased)
            } else {
                to
            }
        };
        (self.sun, self.moon) = (along(self.from[0], sun), along(self.from[1], moon));
        // The stars start dark and fade in, the first frame included.
        self.stars = (self.stars + frame.delta_s * EASE_RATE).min(1.0);
        // Height over the planet: the world is tangent to it at the camera center
        // and the planet falls away under a camera standing off it.
        let km_per_u = km_per_u();
        let r = globe().radius_u();
        let over = v3::len(&[frame.camera[0], frame.camera[1], frame.camera[2] + r]) - r;
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
        self.eased < 1.0 || self.stars < 1.0
    }
}
