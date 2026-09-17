// pxl_core/module/map/environment.rs

//! The environment. The first half is its inputs: the time of day's theme when
//! it changes, and each tick where the map is and what the wall clock says. The
//! second places the sun and the moon (celestial_position.rs), eases the light,
//! the fog and the sky toward the theme, and writes what the renderer draws them
//! with.

use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::module::camera::config::camera_cfg;
use crate::module::map::navigation::use_map_bootstrap::FALLBACK_G4326;
use crate::module::map::store::use_map_store;
use crate::module::world::change::request_world_tick;
use crate::module::world::environment::celestial_position::{moon_position, sun_position};
use crate::module::world::environment::config::{light_cfg, render_cfg};
use crate::module::world::environment::theme::{themes, WorldThemeKey};
use crate::module::world::environment::theme_clock::use_theme_key;
use crate::module::world::sched::chunk_table::Frame;
use crate::render::gpu::color::linear_rgb;
use crate::sys::global::Global;
use crate::sys::host::wall_now;
use crate::sys::kernel::kernel;

// The map's own fallback, so the first frames before map_center exists light
// the world from a real place instead of computing sun and moon for (0, 0).

/// The sky is the inside of a sphere centred on the world origin, and must stay
/// inside the camera's far clip plane. The camera itself can sit up to MAX_DIST
/// away from that origin, and looking toward the far side of the sphere from an
/// off-centre point adds that same distance again; leaving out the margin clips
/// the sky exactly where a wide-shot camera is most likely to be looking.
pub const SKY_DOME_SCALE: f64 = (render_cfg::FAR_CLIP - camera_cfg::view::MAX_DIST) * 0.9;

// The sun and the moon move with the wall clock, a full sweep every 24 real
// hours, and nothing else asks for a frame while the view is still. A
// transition between themes asks for its own frames through the scheduler's
// tick; the clock only needs one this often.
const ENV_CLOCK_FRAME_MS: f64 = 2000.0;

/// The theme as the environment takes it: colours in linear light.
pub fn write_theme(lane: &mut [f32; THEME_FLOATS], key: WorldThemeKey) {
    let t = themes(key);
    let mut k = 0;
    let mut rgb = |lane: &mut [f32; THEME_FLOATS], packed: u32| {
        for v in linear_rgb(packed) {
            lane[k] = v as f32;
            k += 1;
        }
    };
    rgb(lane, t.fog);
    rgb(lane, t.sky.top);
    rgb(lane, t.sky.middle);
    rgb(lane, t.sky.horizon);
    rgb(lane, t.directional.color);
    rgb(lane, t.ambient.color);
    // The rest, in order: the hemisphere's sky and ground and intensity, the
    // directional colour and intensity, the night flag, the fog's range and the
    // light floor.
    let mut at = k;
    let mut put = |lane: &mut [f32; THEME_FLOATS], v: f64| {
        lane[at] = v as f32;
        at += 1;
    };
    put(lane, t.ambient.intensity);
    for v in linear_rgb(t.hemisphere.sky) {
        put(lane, v);
    }
    for v in linear_rgb(t.hemisphere.ground) {
        put(lane, v);
    }
    put(lane, t.hemisphere.intensity);
    for v in linear_rgb(t.directional.color) {
        put(lane, v);
    }
    put(lane, t.directional.intensity);
    put(lane, if key == WorldThemeKey::Night { 1.0 } else { 0.0 });
    put(lane, render_cfg::FOG_NEAR);
    put(lane, render_cfg::FOG_FAR);
    put(lane, light_cfg::FLOOR);
}

struct Environment {
    theme_key: Option<WorldThemeKey>,
    observed_centre: Option<PVF3>,
    observed_lat: f64,
    observed_lon: f64,
    clock_frame_at: f64,
}

static ENVIRONMENT: Global<Environment> = Global::new();

/// The environment's turn, in the interaction phase: before the chunks' step,
/// which runs the scheduler's pass.
pub fn map_environment_step() {
    let e = ENVIRONMENT.get(|| Environment {
        theme_key: None,
        observed_centre: None,
        observed_lat: FALLBACK_G4326[1],
        observed_lon: FALLBACK_G4326[0],
        clock_frame_at: 0.0,
    });
    let sched = &mut kernel().sched;

    let theme_key = use_theme_key();
    if e.theme_key != Some(theme_key) {
        e.theme_key = Some(theme_key);
        let mut lane = [0.0f32; THEME_FLOATS];
        write_theme(&mut lane, theme_key);
        sched.environment.set_theme(&lane);
        request_world_tick();
    }

    let centre = use_map_store().map_center;
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
        request_world_tick();
    }
}

// ---- the environment as the kernel eases it ----
//
// Where the sun and the moon are, and the light, fog and sky colours the
// frame is drawn under, eased toward the time of day's theme. The inputs come
// from above — the theme's colours when the theme changes, and each tick the
// observer's latitude and longitude and the wall clock; this places the sun
// and the moon from the clock, eases everything toward its target, turns the sky's copies of
// the directions by the map's azimuth, and writes the result in the layout
// the renderer's uniforms take (render/gpu/stage.rs).
//
// The terrain's light is the true, unturned direction: the world turns
// under a fixed camera by the azimuth, and a light turned with it would
// cancel the turn out. The sky is drawn in the camera's frame, where the
// sun has to turn with the world to stay over the ground it lights.

/// Floats a theme takes, linear colours: fog (3), sky top, middle and
/// horizon (9), sun glow (3), ambient colour and intensity (4), hemisphere
/// sky and ground and intensity (7), sun colour and intensity (4), night
/// (1), fog near and far (2), the lighting floor (1).
pub const THEME_FLOATS: usize = 34;
/// Floats the output takes: the frame uniform's lighting run (24) and the
/// sky uniform's colours and directions (24), each aligned to the renderer's
/// own layout from its twentieth float; then the second light (8): the
/// dimmer of the sun and the moon, its direction and its colour.
pub const ENV_FLOATS: usize = 56;
/// Where the second light starts in the output.
pub const ENV_OTHER_LIGHT: usize = 48;

/// Where the colours of a theme sit in its floats.
const FOG: usize = 0;
const SKY_TOP: usize = 3;
const SKY_MIDDLE: usize = 6;
const SKY_HORIZON: usize = 9;
const SUN_GLOW: usize = 12;
const AMBIENT: usize = 15;
const HEMI_SKY: usize = 19;
const HEMI_GROUND: usize = 22;
const HEMI_INTENSITY: usize = 25;
const SUN: usize = 26;
const NIGHT: usize = 30;
const FOG_NEAR: usize = 31;
const FOG_FAR: usize = 32;
const FLOOR: usize = 33;

/// The moon's light: a fixed colour (#9fb4d6, linear), brighter than the
/// night theme's sun so that at night it is the light the terrain takes.
const MOON_COLOR: [f32; 3] = [0.3467, 0.4564, 0.6724];
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
    /// Days since J2000.0, from the glue's wall clock.
    days: f64,
    sun: [f64; 3],
    moon: [f64; 3],
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
        self.write(frame);
        (self.stars - star_target).abs() > SETTLE_EPS
    }

    fn write(&mut self, frame: &Frame) {
        let c = &self.current;
        let o = &mut self.out;
        // The terrain is lit by both the sun and the moon, summed: the
        // brighter is the frame's sun, which casts the shadow, and the other
        // is the second light, unshadowed. Summed rather than the brighter
        // alone, so the light does not snap from one direction to the other
        // where their intensities cross during a dusk or dawn ease.
        let sun_i = c[SUN + 3] as f64;
        let sun = (self.sun, [c[SUN], c[SUN + 1], c[SUN + 2]], sun_i);
        let moon = (self.moon, MOON_COLOR, self.moon_intensity);
        let (first, other) = if self.moon_intensity > sun_i { (moon, sun) } else { (sun, moon) };
        for (at, (dir, color, intensity)) in [(0, first), (ENV_OTHER_LIGHT, other)] {
            let d = normalize(dir);
            put3(o, at, [d[0] as f32, d[1] as f32, d[2] as f32]);
            put3(o, at + 4, [color[0] * intensity as f32, color[1] * intensity as f32, color[2] * intensity as f32]);
        }
        o[7] = c[FOG_NEAR];
        let ambient_i = c[AMBIENT + 3];
        put3(o, 8, [c[AMBIENT] * ambient_i, c[AMBIENT + 1] * ambient_i, c[AMBIENT + 2] * ambient_i]);
        o[11] = c[FOG_FAR];
        let hemi_i = c[HEMI_INTENSITY];
        put3(o, 12, [c[HEMI_SKY] * hemi_i, c[HEMI_SKY + 1] * hemi_i, c[HEMI_SKY + 2] * hemi_i]);
        o[15] = c[FLOOR];
        put3(o, 16, [c[HEMI_GROUND] * hemi_i, c[HEMI_GROUND + 1] * hemi_i, c[HEMI_GROUND + 2] * hemi_i]);
        put3(o, 20, [c[FOG], c[FOG + 1], c[FOG + 2]]);

        // The sky, turned by the map's azimuth: the anchor root's turn about Z.
        let az = frame.root[1].atan2(frame.root[0]);
        put3(o, 24, [c[SKY_TOP], c[SKY_TOP + 1], c[SKY_TOP + 2]]);
        o[27] = self.time as f32;
        put3(o, 28, [c[SKY_MIDDLE], c[SKY_MIDDLE + 1], c[SKY_MIDDLE + 2]]);
        o[31] = self.stars as f32;
        put3(o, 32, [c[SKY_HORIZON], c[SKY_HORIZON + 1], c[SKY_HORIZON + 2]]);
        o[35] = az as f32;
        put3(o, 36, turned(normalize(self.sun), az));
        put3(o, 40, [c[SUN_GLOW], c[SUN_GLOW + 1], c[SUN_GLOW + 2]]);
        put3(o, 44, turned(normalize(self.moon), az));
    }
}

fn put3(o: &mut [f32; ENV_FLOATS], at: usize, v: [f32; 3]) {
    o[at..at + 3].copy_from_slice(&v);
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn turned(v: [f64; 3], az: f64) -> [f32; 3] {
    let (s, c) = az.sin_cos();
    [(v[0] * c - v[1] * s) as f32, (v[0] * s + v[1] * c) as f32, v[2] as f32]
}

#[cfg(test)]
#[path = "environment.test.rs"]
mod test;
