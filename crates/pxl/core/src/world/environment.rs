//! The environment: where the sun and the moon are, and the light, fog and
//! sky colours the frame is drawn under, eased toward the time of day's
//! theme. module/map/environment.rs hands over the inputs — the theme's
//! colours when the theme changes, and each tick the observer's latitude
//! and longitude and the wall clock; this places the sun and the moon from
//! the clock, eases everything toward its target, turns the sky's copies of
//! the directions by the map's azimuth, and writes the result in the layout
//! the renderer's uniforms take (render/gpu/stage.rs).
//!
//! The terrain's light is the true, unturned direction: the world turns
//! under a fixed camera by the azimuth, and a light turned with it would
//! cancel the turn out. The sky is drawn in the camera's frame, where the
//! sun has to turn with the world to stay over the ground it lights.

use crate::world::sched::Frame;

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

pub struct Environment {
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

impl Default for Environment {
    fn default() -> Self {
        Environment {
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

impl Environment {
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

// ---- where the sun and the moon are ----
//
// The standard low-precision formulas: the sun good to about 0.01°, the
// moon's single dominant term per angle to roughly 0.3–1°. Degrees until
// the local direction, as they are conventionally published.

const DEG: f64 = core::f64::consts::PI / 180.0;

fn norm360(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// The sun's right ascension and declination, and the obliquity, in degrees.
fn sun_equatorial(d: f64) -> (f64, f64, f64) {
    let mean_long = norm360(280.46 + 0.985_647_4 * d);
    let g = norm360(357.528 + 0.985_600_3 * d) * DEG;
    let lon = norm360(mean_long + 1.915 * g.sin() + 0.02 * (2.0 * g).sin()) * DEG;
    let obliquity = 23.439 - 0.000_000_4 * d;
    let e = obliquity * DEG;
    let ra = norm360((e.cos() * lon.sin()).atan2(lon.cos()) / DEG);
    let dec = (e.sin() * lon.sin()).asin() / DEG;
    (ra, dec, obliquity)
}

/// The moon's right ascension and declination, in degrees.
fn moon_equatorial(d: f64, obliquity: f64) -> (f64, f64) {
    let mean_long = norm360(218.316 + 13.176_396 * d);
    let mean_anomaly = norm360(134.963 + 13.064_993 * d);
    let arg_latitude = norm360(93.272 + 13.229_35 * d);
    let lon = norm360(mean_long + 6.289 * (mean_anomaly * DEG).sin()) * DEG;
    let lat = 5.128 * (arg_latitude * DEG).sin() * DEG;
    let e = obliquity * DEG;
    let ra = norm360((lon.sin() * e.cos() - lat.tan() * e.sin()).atan2(lon.cos()) / DEG);
    let dec = (lat.sin() * e.cos() + lat.cos() * e.sin() * lon.sin()).asin() / DEG;
    (ra, dec)
}

/// Equatorial to the observer's local direction: X east, Y north, Z up, at
/// distance 100 as the three lights stood. Azimuth is the Duffett-Smith
/// construction, acos and a sign from the hour angle, as the glue had it.
fn local_direction(ra: f64, dec: f64, lat: f64, lon: f64, d: f64) -> [f64; 3] {
    let gmst = norm360(280.460_618_37 + 360.985_647_366_29 * d);
    let lst = norm360(gmst + lon);
    let hour_angle = (lst - ra + 180.0).rem_euclid(360.0) - 180.0;
    let (ha, dec, lat) = (hour_angle * DEG, dec * DEG, lat * DEG);
    let alt = (dec.sin() * lat.sin() + dec.cos() * lat.cos() * ha.cos()).asin();
    let cos_az = ((dec.sin() - alt.sin() * lat.sin()) / (alt.cos() * lat.cos())).clamp(-1.0, 1.0);
    let mut az = cos_az.acos();
    if ha.sin() > 0.0 {
        az = 2.0 * core::f64::consts::PI - az;
    }
    let horizontal = alt.cos() * 100.0;
    [az.sin() * horizontal, az.cos() * horizontal, alt.sin() * 100.0]
}

pub fn sun_position(d: f64, lat: f64, lon: f64) -> [f64; 3] {
    let (ra, dec, _) = sun_equatorial(d);
    local_direction(ra, dec, lat, lon, d)
}

pub fn moon_position(d: f64, lat: f64, lon: f64) -> [f64; 3] {
    let (_, _, obliquity) = sun_equatorial(d);
    let (ra, dec) = moon_equatorial(d, obliquity);
    local_direction(ra, dec, lat, lon, d)
}

#[cfg(test)]
#[path = "environment.test.rs"]
mod test;
