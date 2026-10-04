// pxl-kernel/layer/environment/state.rs

//! The environment: the theme, where the map is and the wall clock as
//! inputs; the sun and moon placed and the light, fog and sky eased toward
//! the theme.

use pxl_render::look::color::linear_rgb;
use pxl_render::look::environment::{write_environment, Light, ENV_FLOATS, NIGHT, THEME_FLOATS};
use pxl_render::look::globe::Globe;
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{len3, V3};

use crate::control::navigation::bootstrap::FALLBACK_G4326;
use crate::host::wall_now;
use crate::kernel::kernel;
use crate::layer::environment::celestial_position::{equatorial_to_local, j2000_days, moon_position, sun_position};
use crate::layer::environment::config::light_cfg;
use crate::layer::environment::fog::{band_at, eye_ground, eye_height, reach_at};
use crate::layer::environment::theme::{themes, Theme};
use crate::layer::environment::theme_clock::theme_key;
use crate::world::change::{request_world_tick, wake_at};
use crate::world::chunk::table::Frame;
use crate::world::frame::view::globe_anchor;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;

// The sky moves with the wall clock and nothing else asks for a frame while
// the view is still, so the clock asks for one this often.
const ENV_CLOCK_FRAME_MS: f64 = 2000.0;

/// Colors in linear light.
pub fn write_theme(lane: &mut [f32; THEME_FLOATS], key: Theme) {
    let t = themes(key);
    let mut k = 0;
    let mut rgb = |lane: &mut [f32; THEME_FLOATS], packed: u32| {
        for v in linear_rgb(packed) {
            lane[k] = v as f32;
            k += 1;
        }
    };
    rgb(lane, t.ambient.color);
    lane[k] = t.ambient.intensity as f32;
    lane[k + 1] = if key == Theme::Night { 1.0 } else { 0.0 };
}

struct Environment {
    theme_key: Option<Theme>,
    observed_center: Option<Pvf3>,
    observed_lat: f64,
    observed_lon: f64,
    clock_frame_at: f64,
}

static ENVIRONMENT: Global<Environment> = Global::new();

fn environment() -> &'static mut Environment {
    ENVIRONMENT.get(|| Environment {
        theme_key: None,
        observed_center: None,
        observed_lat: FALLBACK_G4326[1],
        observed_lon: FALLBACK_G4326[0],
        clock_frame_at: 0.0,
    })
}

/// Mercator stretches by the secant of the latitude, heights alike.
pub fn km_per_u() -> f64 {
    epsg3857::M_PER_U * environment().observed_lat.to_radians().cos() / 1000.0
}

pub fn globe() -> Globe {
    Globe::at(environment().observed_lat.to_radians())
}

/// Runs before the chunks' step.
pub fn map_environment_step() {
    let e = environment();
    let sched = &mut kernel().sched;

    let theme_key = theme_key();
    if e.theme_key != Some(theme_key) {
        e.theme_key = Some(theme_key);
        let mut lane = [0.0f32; THEME_FLOATS];
        write_theme(&mut lane, theme_key);
        sched.environment.set_theme(&lane);
        request_world_tick();
    }

    // Observed from the globe's anchor, which the world is laid on the planet about.
    let center = globe_anchor();
    if center != e.observed_center {
        e.observed_center = center;
        if let Some(c) = center {
            let g = epsg4326::unproject(c.i, c.f);
            e.observed_lon = g[0];
            e.observed_lat = g[1];
        }
    }
    let unix_ms = wall_now();
    sched.environment.observe(unix_ms);

    if unix_ms - e.clock_frame_at >= ENV_CLOCK_FRAME_MS {
        e.clock_frame_at = unix_ms;
    }
    wake_at(now() + e.clock_frame_at + ENV_CLOCK_FRAME_MS - unix_ms);
}

const MOON_NIGHT_INTENSITY: f64 = 0.18;
const MOON_START_INTENSITY: f64 = 0.3;
/// Below this the transition counts as done and stops asking for frames.
const SETTLE_EPS: f64 = 0.005;
/// A theme change lands in about 1.5 s and never steps.
const EASE_RATE: f64 = 1.5;

pub struct EnvironmentDraw {
    target: [f32; THEME_FLOATS],
    current: [f32; THEME_FLOATS],
    has_theme: bool,
    started: bool,
    /// Days since J2000.0, from the host's wall clock.
    days: f64,
    sun: V3,
    moon: V3,
    moon_intensity: f64,
    stars: f64,
    /// Seconds of wall time since the first observation: a tick's delta is
    /// clamped and sparse at rest, which ran clouds and stars slow, and the
    /// epoch would not survive the shader's f32.
    time: f64,
    first_ms: Option<f64>,
    pub out: [f32; ENV_FLOATS],
    /// Sunrise and sunset, in hours after the local day's start.
    pub daylight: [f64; 2],
}

impl Default for EnvironmentDraw {
    fn default() -> Self {
        EnvironmentDraw {
            target: [0.0; THEME_FLOATS],
            current: [0.0; THEME_FLOATS],
            has_theme: false,
            started: false,
            days: 0.0,
            sun: [0.0; 3],
            moon: [0.0; 3],
            moon_intensity: 0.0,
            stars: 0.0,
            time: 0.0,
            first_ms: None,
            out: [0.0; ENV_FLOATS],
            daylight: [6.0, 18.0],
        }
    }
}

impl EnvironmentDraw {
    /// The first theme is taken at once.
    pub fn set_theme(&mut self, theme: &[f32; THEME_FLOATS]) {
        self.target = *theme;
        if !self.has_theme {
            self.current = *theme;
            self.has_theme = true;
        }
    }

    /// Unix milliseconds.
    pub fn observe(&mut self, unix_ms: f64) {
        self.days = j2000_days(unix_ms);
        let first = *self.first_ms.get_or_insert(unix_ms);
        self.time = (unix_ms - first) / 1000.0;
    }

    /// Sampled every five minutes from the same sun_position formula rather
    /// than a closed-form sunrise equation that could disagree with it.
    /// 06:00 and 18:00 where the sun does not cross the horizon.
    pub fn daylight(&mut self, lat: f64, lon: f64, day_start_ms: f64) {
        const SAMPLES: u32 = 288;
        let altitude = |hour: f64| {
            let d = j2000_days(day_start_ms + hour * 3_600_000.0);
            let p = sun_position(d, lat, lon);
            p[2]
        };
        let step = 24.0 / SAMPLES as f64;
        let (mut rise, mut set) = (None, None);
        let mut prev = altitude(0.0);
        for i in 1..=SAMPLES {
            let hour = i as f64 * step;
            let alt = altitude(hour);
            if rise.is_none() && prev < 0.0 && alt >= 0.0 {
                rise = Some(hour - step * (1.0 - -prev / (alt - prev)));
            }
            if set.is_none() && rise.is_some() && prev >= 0.0 && alt < 0.0 {
                set = Some(hour - step * (1.0 - prev / (prev - alt)));
            }
            prev = alt;
        }
        self.daylight = [rise.unwrap_or(6.0), set.unwrap_or(18.0)];
    }

    /// Whether a transition is still in flight and wants frames.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        if !self.has_theme {
            return false;
        }
        let night = self.target[NIGHT] > 0.5;
        let (lat, lon) = (environment().observed_lat, environment().observed_lon);
        let sun = sun_position(self.days, lat, lon);
        let moon = moon_position(self.days, lat, lon);
        let a = if self.started { (frame.delta_s * EASE_RATE).clamp(0.0, 1.0) } else { 1.0 };
        if !self.started {
            self.sun = sun;
            self.moon = moon;
            self.moon_intensity = if night { MOON_START_INTENSITY } else { 0.0 };
            self.started = true;
        }
        for k in 0..3 {
            self.sun[k] += (sun[k] - self.sun[k]) * a;
            self.moon[k] += (moon[k] - self.moon[k]) * a;
        }
        let star_target = if night { 1.0 } else { 0.0 };
        // The stars start dark and fade in, the first frame included.
        let star_a = (frame.delta_s * EASE_RATE).clamp(0.0, 1.0);
        self.stars += (star_target - self.stars) * star_a;
        self.moon_intensity += ((if night { MOON_NIGHT_INTENSITY } else { 0.0 }) - self.moon_intensity) * a;
        for k in 0..THEME_FLOATS {
            self.current[k] += (self.target[k] - self.current[k]) * a as f32;
        }
        // Height over the planet: the world is tangent to it at the camera center
        // and the planet falls away under a camera standing off it.
        let km_per_u = km_per_u();
        let r = globe().radius_u();
        let over = len3(&[frame.camera[0], frame.camera[1], frame.camera[2] + r]) - r;
        let height = eye_height();
        // The ground pyramid's last observation, read rather than remade per frame.
        let drawn = crate::world::pyramid::domain::current().map(|d| d.observation().far);
        let band = band_at(height, drawn.unwrap_or_else(|| reach_at(height)));
        let light = Light {
            theme: self.current,
            sun: self.sun,
            moon: self.moon,
            moon_intensity: self.moon_intensity,
            stars: self.stars,
            time: self.time,
            fog_near: band.0,
            fog_far: band.1,
            floor: light_cfg::FLOOR,
            altitude_km: (frame.center_z + over) * km_per_u,
            ground_km: eye_ground() * km_per_u,
            km_per_u,
            latitude: lat.to_radians(),
            star_frame: equatorial_to_local(self.days, lat, lon),
        };
        write_environment(&mut self.out, &light);
        (self.stars - star_target).abs() > SETTLE_EPS
    }
}

#[cfg(test)]
#[path = "state.test.rs"]
mod test;
