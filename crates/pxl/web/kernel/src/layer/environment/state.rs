// pxl-kernel/layer/environment/state.rs

//! The environment. The first half is its inputs: the time of day's theme when
//! it changes, and each tick where the map is and what the wall clock says. The
//! second places the sun and the moon (celestial_position.rs) and eases the
//! light, the fog and the sky toward the theme; the view composes the lane the
//! frame is lit with from that (crates/pxl/web/render/src/look/environment.rs).

use pxl_render::look::color::linear_rgb;
use pxl_render::look::environment::{write_environment, Light, ENV_FLOATS, NIGHT, THEME_FLOATS};
use pxl_render::look::globe::Globe;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::control::navigation::bootstrap::FALLBACK_G4326;
use crate::global::Global;
use crate::host::{now, wall_now};
use crate::kernel::kernel;
use crate::layer::environment::celestial_position::{equatorial_to_local, moon_position, sun_position};
use crate::layer::environment::config::light_cfg;
use crate::layer::environment::fog::{band_at, eye_ground, eye_height, reach_at};
use crate::layer::environment::theme::{themes, Theme};
use crate::layer::environment::theme_clock::theme_key;
use crate::shell::map_store::map_store;
use crate::world::change::{request_world_tick, wake_at};
use crate::world::chunk::table::Frame;
use crate::world::frame::view::camera_centre;
use crate::world::pyramid::Layer as _;

// The map's own fallback, so the first frames before map_center exists light
// the world from a real place instead of computing sun and moon for (0, 0).

// The sun and the moon move with the wall clock, a full sweep every 24 real
// hours, and nothing else asks for a frame while the view is still. A
// transition between themes asks for its own frames through the scheduler's
// tick; the clock only needs one this often.
const ENV_CLOCK_FRAME_MS: f64 = 2000.0;

/// The theme as the environment takes it: colours in linear light.
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
    // The rest, in order: the ambient's intensity and the night flag.
    lane[k] = t.ambient.intensity as f32;
    lane[k + 1] = if key == Theme::Night { 1.0 } else { 0.0 };
}

struct Environment {
    theme_key: Option<Theme>,
    observed_centre: Option<Pvf3>,
    observed_lat: f64,
    observed_lon: f64,
    clock_frame_at: f64,
}

static ENVIRONMENT: Global<Environment> = Global::new();

fn environment() -> &'static mut Environment {
    ENVIRONMENT.get(|| Environment {
        theme_key: None,
        observed_centre: None,
        observed_lat: FALLBACK_G4326[1],
        observed_lon: FALLBACK_G4326[0],
        clock_frame_at: 0.0,
    })
}

/// A world unit's true length where the map is, in kilometres: the
/// projection's, which Mercator stretches by the secant of the latitude, in
/// either axis alike since heights are stretched with it.
pub fn km_per_u() -> f64 {
    epsg3857::M_PER_U * environment().observed_lat.to_radians().cos() / 1000.0
}

/// The sphere about the camera centre (view.rs's camera_centre), which the
/// flat map is laid back on (pxl-render's look/globe.rs), for the chunks'
/// culling as the renderer takes it for its frame.
pub fn globe() -> Globe {
    Globe::at(environment().observed_lat.to_radians())
}

/// The environment's turn, in the interaction phase: before the chunks' step,
/// which runs the scheduler's pass.
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

    // Observed from the camera centre, the one the world is laid on the
    // planet about, so the sphere's latitude and the sky over it are its.
    let centre = map_store().map_center.map(|_| camera_centre());
    if centre != e.observed_centre {
        e.observed_centre = centre;
        if let Some(c) = centre {
            let mut g = [0.0; 3];
            epsg4326::unproject(&mut g, c.i, c.f);
            e.observed_lon = g[0];
            e.observed_lat = g[1];
        }
    }
    let unix_ms = wall_now();
    sched.environment.observe(e.observed_lat, e.observed_lon, unix_ms);

    if unix_ms - e.clock_frame_at >= ENV_CLOCK_FRAME_MS {
        e.clock_frame_at = unix_ms;
    }
    wake_at(now() + e.clock_frame_at + ENV_CLOCK_FRAME_MS - unix_ms);
}

// ---- the environment as the kernel eases it ----
//
// Where the sun and the moon are, and the theme's light, fog and sky colours,
// eased toward the time of day's theme. The inputs come from above — the
// theme's colours when the theme changes, and each tick the observer's
// latitude and longitude and the wall clock; this places the sun and the moon
// from the clock and eases everything toward its target. What the frame is lit
// with is composed from that by the view (crates/pxl/web/render/src/look/environment.rs).

/// The moon's intensity at night, and where it starts on a first night.
const MOON_NIGHT_INTENSITY: f64 = 0.18;
const MOON_START_INTENSITY: f64 = 0.3;
/// How close the stars have to be to where they are going before the
/// transition counts as done and the tick stops asking for frames.
const SETTLE_EPS: f64 = 0.005;
/// The easing: each frame closes `delta * 1.5` of the gap to the target, so
/// a theme change lands in about a second and a half and never steps.
const EASE_RATE: f64 = 1.5;

pub struct EnvironmentDraw {
    target: [f32; THEME_FLOATS],
    current: [f32; THEME_FLOATS],
    has_theme: bool,
    started: bool,
    lat: f64,
    lon: f64,
    /// Days since J2000.0, from the host's wall clock.
    days: f64,
    sun: V3,
    moon: V3,
    moon_intensity: f64,
    stars: f64,
    /// The sky's clock, seconds of wall time since the first observation:
    /// the clouds drift and the stars twinkle by it whether or not the
    /// tick runs, and a tick's delta — clamped, and asked for every two
    /// seconds at rest — would run them at a fraction of the speed. From
    /// the first observation rather than the epoch, so that it survives
    /// the f32 the shader reads.
    time: f64,
    first_ms: Option<f64>,
    pub out: [f32; ENV_FLOATS],
    /// The last `daylight` asked for: sunrise and sunset, in hours after the
    /// start of the local day.
    pub daylight: [f64; 2],
}

impl Default for EnvironmentDraw {
    fn default() -> Self {
        EnvironmentDraw {
            target: [0.0; THEME_FLOATS],
            current: [0.0; THEME_FLOATS],
            has_theme: false,
            started: false,
            lat: 0.0,
            lon: 0.0,
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
    /// The theme the environment eases toward; the first one is taken at once.
    pub fn set_theme(&mut self, theme: &[f32; THEME_FLOATS]) {
        self.target = *theme;
        if !self.has_theme {
            self.current = *theme;
            self.has_theme = true;
        }
    }

    /// The observer, in degrees, and the wall clock, in Unix milliseconds.
    pub fn observe(&mut self, lat: f64, lon: f64, unix_ms: f64) {
        self.lat = lat;
        self.lon = lon;
        self.days = unix_ms / 86_400_000.0 + 2_440_587.5 - 2_451_545.0;
        let first = *self.first_ms.get_or_insert(unix_ms);
        self.time = (unix_ms - first) / 1000.0;
    }

    /// Sunrise and sunset for the local day starting at `day_start_ms` (Unix
    /// milliseconds), in hours after it — the split the theme clock keys the
    /// day's and the night's themes by.
    ///
    /// Sampled rather than solved: the sun's altitude every five minutes,
    /// 288 of them, and the zero crossings interpolated. A closed-form
    /// sunrise equation would be a second formula that could disagree with
    /// the position one above it; this cannot, because it is that formula.
    /// 06:00 and 18:00 where the sun does not cross the horizon that day.
    pub fn daylight(&mut self, lat: f64, lon: f64, day_start_ms: f64) {
        const SAMPLES: u32 = 288;
        let altitude = |hour: f64| {
            let d = (day_start_ms + hour * 3_600_000.0) / 86_400_000.0 + 2_440_587.5 - 2_451_545.0;
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

    /// Advances the environment a frame; whether a transition is still in
    /// flight and wants frames.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        if !self.has_theme {
            return false;
        }
        let night = self.target[NIGHT] > 0.5;
        let sun = sun_position(self.days, self.lat, self.lon);
        let moon = moon_position(self.days, self.lat, self.lon);
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
        // The camera's height over the planet, measured from its centre,
        // under the camera centre by the radius (look/globe.rs): the world is
        // tangent to the planet at the camera centre, and the planet falls
        // away under a camera standing off it. Above the sea, it is that and
        // the camera centre's height.
        let km_per_u = km_per_u();
        let r = Globe::at(self.lat.to_radians()).radius_u();
        let over = frame.camera[0].hypot(frame.camera[1]).hypot(frame.camera[2] + r) - r;
        // Where the ground ends for the eye: the nearer of what the air and
        // the planet leave (environment/fog.rs) and what the window draws.
        let height = eye_height();
        // What the ground's pyramid last drew, the view it was made from:
        // read, not made again a frame at a time.
        let drawn = Terrain::pyramid().domain().map(|d| d.far);
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
            altitude_km: (frame.centre_z + over) * km_per_u,
            ground_km: eye_ground() * km_per_u,
            km_per_u,
            latitude: self.lat.to_radians(),
            star_frame: equatorial_to_local(self.days, self.lat, self.lon),
        };
        write_environment(&mut self.out, &light);
        (self.stars - star_target).abs() > SETTLE_EPS
    }
}

#[cfg(test)]
#[path = "state.test.rs"]
mod test;
