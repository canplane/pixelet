// pxl-kernel/domain/scene/environment.rs

//! Time of day and the light it gives: where the world is anchored and the
//! wall clock as inputs; the sun and moon placed, and the light and sky they
//! give.

pub mod celestial;
pub mod cloud;
pub mod param;

use pxl_content::cloud::drift;
use pxl_content::param::night;
use pxl_render::gpu::renderer::sky_steps;
use pxl_render::look::environment::{write_environment, Light, ENV_FLOATS};
use pxl_spec::space::coord::v3;

use crate::domain::scene::animation;
use crate::domain::scene::environment::param::SKY_EASE_RATE;
use crate::domain::scene::frame::Frame;
use crate::domain::view::frame::pose::{anchor, km_per_u};
use crate::domain::view::lens::Stepped;
use crate::domain::view::observation;
use crate::domain::view::setting::schema_spec::SkyTime;
use crate::domain::view::setting::store::settings_store;
use crate::sys::sched::clock::wall_now;
use crate::sys::sched::edge::wake_at;
use celestial::{equatorial_to_local, j2000_days, moon_pos, sun_pos};
use xpute_core::clock::now;
use xpute_core::global::Global;

/// Runs before the markers' step: the sky moves with the wall clock and
/// nothing else asks for a frame while the view is still, so the clock asks
/// for one when its next step falls (`EnvironmentDraw::observe`).
pub fn environment_step() {
    let next_ms = environment().observe(wall_now());
    wake_at(now() + next_ms);
}

static ENVIRONMENT: Global<EnvironmentDraw> = Global::new();

fn environment() -> &'static mut EnvironmentDraw {
    ENVIRONMENT.get(EnvironmentDraw::default)
}

/// The sky's transitions advanced (`EnvironmentDraw::tick`); whether one is
/// in flight.
pub fn environment_animate(frame: &Frame) -> bool {
    environment().tick(frame)
}

/// The sun under the horizon, as drawn (`EnvironmentDraw::night`).
pub fn night() -> bool {
    environment().night()
}

/// The environment's words as the last frame wrote them, which the renderer
/// draws the sky and the light by (pxl-render look/environment.rs).
pub fn environment_words() -> &'static [f32; ENV_FLOATS] {
    &environment().out
}

/// How far the wind has carried the clouds (`EnvironmentDraw::drift`).
pub fn environment_drift() -> [f64; 2] {
    environment().drift
}

/// The eye's height over the sea (`EnvironmentDraw::alt_km`).
pub fn environment_alt_km() -> f64 {
    environment().alt_km
}

/// The cosine of the sun's and the moon's zenith angles at `days`, seen from
/// `[lon, lat]`: each one's direction's up, the positions standing at a
/// distance.
fn heights(days: f64, [lon, lat]: [f64; 2]) -> [f64; 2] {
    let mu = |p: [f64; 3]| p[2] / v3::len(&p);
    [mu(sun_pos(days, lat, lon)), mu(moon_pos(days, lat, lon))]
}

/// The Unix ms the sky is drawn at, `unix_ms` the wall clock's, seen from
/// longitude `lon` in degrees (the `sky_time` setting): the wall clock's
/// own, or the hour `time` names in the place's mean solar time today, held
/// there, so the sun stands where it does at that hour whatever the clock
/// reads; and whether it is held.
fn sky_clock(time: SkyTime, unix_ms: f64, lon: f64) -> (f64, bool) {
    const HOUR_MS: f64 = 3_600_000.0;
    let hour = match time {
        SkyTime::real => return (unix_ms, false),
        SkyTime::dawn => 7.0,
        SkyTime::morning => 9.0,
        SkyTime::noon => 12.0,
        SkyTime::afternoon => 15.0,
        SkyTime::dusk => 17.0,
    };
    // The place's solar time runs ahead of UTC by its longitude, 15° an hour.
    let ahead = lon / 15.0 * HOUR_MS;
    let today = libm::floor((unix_ms + ahead) / (24.0 * HOUR_MS)) * 24.0 * HOUR_MS;
    (today + hour * HOUR_MS - ahead, true)
}

/// Each light's height a display level of the sky moves over, or, out of
/// reach of the air, the height before it comes into reach (pxl-render
/// `sky_steps`); 0 before its first slope lands, so every observation steps
/// until one has.
fn steps() -> [f64; 2] {
    sky_steps().map(f64::from)
}

pub struct EnvironmentDraw {
    started: bool,
    /// Days since J2000.0 the sky is drawn for, from the runtime's wall clock
    /// a step at a time (`observe`).
    days: f64,
    sun: [f64; 3],
    moon: [f64; 3],
    /// Where the sun and moon stand and how the stars turn at the days and
    /// the anchor the sky is seen at, made where either steps (`aim`).
    aimed: [[f64; 3]; 2],
    star_frame: [[f64; 3]; 3],
    /// The anchor the sky is observed from, which follows the eye's only
    /// where the sun or the moon stands a display level higher or lower
    /// from it (`steps`), where they stood when it last moved, and how far
    /// they have come since, to one at the end.
    anchor: [f64; 2],
    from: [[f64; 3]; 2],
    progress: f64,
    stars: f64,
    /// How far the wind has carried the clouds, X and Y in units, by their
    /// clock, the wall time since the first observation, held between their
    /// steps (`observe`).
    drift: [f64; 2],
    /// The eye's height over the sea, in km, as the last frame saw it
    /// (`tick`).
    alt_km: f64,
    /// Where the sky's hour and the clouds' wall clock last stepped, in
    /// Unix ms.
    sky_at: Stepped,
    clouds_at: Stepped,
    first_ms: Option<f64>,
    out: [f32; ENV_FLOATS],
}

impl Default for EnvironmentDraw {
    fn default() -> Self {
        EnvironmentDraw {
            started: false,
            days: 0.0,
            sun: [0.0; 3],
            moon: [0.0; 3],
            aimed: [[0.0; 3]; 2],
            star_frame: [[0.0; 3]; 3],
            anchor: [0.0; 2],
            from: [[0.0; 3]; 2],
            progress: 1.0,
            stars: 0.0,
            drift: [0.0; 2],
            alt_km: 0.0,
            sky_at: Stepped::default(),
            clouds_at: Stepped::default(),
            first_ms: None,
            out: [0.0; ENV_FLOATS],
        }
    }
}

impl EnvironmentDraw {
    /// The anchor the sky is seen from: the eye's (`anchor`) before the sky
    /// starts or where the sun or the moon stands a display level higher or
    /// lower from it, else the one it is seen from.
    fn sky_anchor(&self) -> [f64; 2] {
        let [lon, lat, _] = anchor();
        let live = [lon, lat];
        if !self.started {
            return live;
        }
        let (was, is, step) = (
            heights(self.days, self.anchor),
            heights(self.days, live),
            steps(),
        );
        if (0..2).any(|k| (is[k] - was[k]).abs() >= step[k]) {
            live
        } else {
            self.anchor
        }
    }

    /// The sun under the horizon, as drawn: past where night begins to fall
    /// (pxl-content param.rs `night::FALL`).
    pub fn night(&self) -> bool {
        self.started && v3::norm(&self.sun)[2] < f64::from(night::FALL[1])
    }

    /// Unix milliseconds: the sky's days and the clouds' time, each held and
    /// stepped a quantum at a time, and how many milliseconds on the earlier
    /// of their next steps falls. A clock steps once its next step's time is
    /// reached, that time taken where it last stepped: one quantum at the rate
    /// it moved there. The sky's quantum is its step, the height over which
    /// the sky as drawn moves one display level (pxl-render `sky_steps`, its
    /// views' own slopes), or, for a light out of reach of the air, the height
    /// before it comes into reach, at the rate the sun's or the moon's height
    /// moves; a light with no step landed yet asks the least, so every
    /// observation steps until one lands. The clouds' is the wind carrying them the
    /// seen step at the nearest (`cloud::drift_step_s`). Between steps their
    /// words stand (`lens::Stepped`); before the sky starts, every
    /// observation steps them. A clock set back before where a light last
    /// stepped steps it there at once. The sky's clock is the hour
    /// the `sky_time` setting shows (`sky_clock`), which a held hour never steps;
    /// the clouds' is the wall clock's.
    pub fn observe(&mut self, unix_ms: f64) -> f64 {
        let first = *self.first_ms.get_or_insert(unix_ms);
        let at = self.sky_anchor();
        let (sky_ms, held) = sky_clock(settings_store().sky_time, unix_ms, at[0]);
        let heights = |days: f64| heights(days, at);
        let step = steps();
        // As fast as each height moves over the minute from `at`, when it will
        // have moved a step. A light with no step landed (`unknown`) steps at
        // every observation and wakes no turn: its measures' landing does
        // (pxl-render `landed`).
        const MINUTE_MS: f64 = 60_000.0;
        let rate = |at: f64, unknown: f64| {
            let days = j2000_days(at);
            let (now, then) = (heights(days), heights(days + MINUTE_MS / 86_400_000.0));
            at + (0..2)
                .map(|k| match step[k] {
                    0.0 => unknown,
                    s => s * MINUTE_MS / (then[k] - now[k]).abs(),
                })
                .fold(f64::INFINITY, f64::min)
        };
        let sky_next = |at: f64| rate(at, 0.0);
        let clouds_next = |at: f64| at + cloud::drift_step_s() * 1000.0;
        if !self.started {
            (self.sky_at, self.clouds_at) = Default::default();
        }
        let (sky, _, stepped) = self.sky_at.at(sky_ms, sky_next);
        if stepped {
            self.days = j2000_days(sky);
            self.aim();
        }
        let (clouds, clouds_due, drifted) = self.clouds_at.at(unix_ms, clouds_next);
        // The clouds drifting step on their own, waking the turn.
        if drifted && self.started {
            animation::note_animation();
        }
        self.drift = drift((clouds - first) / 1000.0);
        let sky_in = if held {
            f64::INFINITY
        } else {
            rate(sky, f64::INFINITY) - sky_ms
        };
        sky_in.min(clouds_due - unix_ms).max(0.0)
    }

    /// The sun and moon and the stars' turn at the sky's days and anchor.
    fn aim(&mut self) {
        let [lon, lat] = self.anchor;
        self.aimed = [sun_pos(self.days, lat, lon), moon_pos(self.days, lat, lon)];
        self.star_frame = equatorial_to_local(self.days, lat, lon);
    }

    /// Whether a transition is still in flight and wants frames: the sun
    /// and moon on their way across a jump of the anchor, the stars fading
    /// in. The clock's steps are taken as they come (`observe`), a display
    /// level each, so a still sky's words stand between them.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        if self.first_ms.is_none() {
            return false;
        }
        // Observed from the anchor the sky follows: a pan that moves neither
        // light a display level moves no word of the sky.
        let live = self.sky_anchor();
        if !self.started || live != self.anchor {
            if self.started {
                (self.from, self.progress) = ([self.sun, self.moon], 0.0);
            }
            self.anchor = live;
            self.aim();
        }
        let [sun, moon] = self.aimed;
        if !self.started {
            (self.sun, self.moon) = (sun, moon);
            self.started = true;
        }
        self.progress = (self.progress + frame.delta_s * SKY_EASE_RATE).min(1.0);
        let along = |from: [f64; 3], to: [f64; 3]| {
            if self.progress < 1.0 {
                core::array::from_fn(|k| from[k] + (to[k] - from[k]) * self.progress)
            } else {
                to
            }
        };
        (self.sun, self.moon) = (along(self.from[0], sun), along(self.from[1], moon));
        // The stars start dark and fade in, the first frame included.
        self.stars = (self.stars + frame.delta_s * SKY_EASE_RATE).min(1.0);
        // Height over the sea under the eye, which the camera stands over.
        let km_per_u = km_per_u();
        self.alt_km = frame.camera[2] * km_per_u;
        // The ground under the eye, as the turn observed it.
        let ground = observation::observation().map_or(0.0, |o| o.ground);
        let light = Light {
            sun: self.sun,
            moon: self.moon,
            stars: self.stars,
            alt_km: self.alt_km,
            ground_km: ground * km_per_u,
            km_per_u,
            star_frame: self.star_frame,
        };
        write_environment(&mut self.out, &light);
        self.progress < 1.0 || self.stars < 1.0
    }
}
