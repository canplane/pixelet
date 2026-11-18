// pxl-core/domain/scene/environment.rs

//! The sky over the world. From the place the world is anchored at and the
//! wall clock's time, it places the sun, the moon and the stars, and it writes
//! the words the renderer draws the sky and its light from. Those words change
//! only when the drawn sky would change by a display level, one step of a
//! displayed color, so a still sky costs no frames between such changes.

pub(crate) mod celestial;
pub(crate) mod cloud;
pub(crate) mod param;

use crate::spec::step::InvalidationReason;
use pxl_content::cloud::drift;
use pxl_content::param::night;
use pxl_render::look::environment::{FLOATS, Light, write_environment};
use pxl_render::renderer::sky_steps;
use pxl_spec::space::coord::v3;

use crate::domain::scene::animation;
use crate::domain::scene::environment::param::SKY_EASE_RATE;
use crate::domain::scene::frame::Frame;
use crate::domain::setting::settings;
use crate::domain::view::frame::pose::{anchor, km_per_u};
use crate::domain::view::lens::StepClock;
use crate::domain::view::observation;
use crate::spec::schema::SkyTime;
use crate::sys::sched::invalidate_at;
use celestial::{equatorial_to_local, j2000_days, moon_pos, sun_pos};
use pxl_global::Global;
use xpute_guest::clock::now;
use xpute_guest::clock::system;

/// Observes the sky at the wall clock's time and wakes a frame when the next
/// step of the sky or of the clouds falls (`EnvironmentDraw::observe`). The
/// sky moves with the wall clock while nothing else asks for a frame, so
/// without this wake a still view would never show it move.
pub(crate) fn environment_step() {
    let now_ms = now();
    let next_ms = environment().observe(system(), now_ms);
    invalidate_at(now_ms + next_ms, InvalidationReason::Scene);
}

static ENVIRONMENT: Global<EnvironmentDraw> = Global::new();

fn environment() -> &'static mut EnvironmentDraw {
    ENVIRONMENT.get(EnvironmentDraw::default)
}

/// Advances the sky by one frame (`EnvironmentDraw::tick`). Returns whether
/// a transition still wants frames.
pub(crate) fn environment_animate(frame: &Frame) -> bool {
    environment().tick(frame)
}

/// Whether the sun, as drawn, is under the horizon (`EnvironmentDraw::night`).
pub(crate) fn night() -> bool {
    environment().night()
}

/// The environment's words as the last frame wrote them. The renderer draws
/// the sky and its light from them (pxl-render look/environment.rs).
pub(crate) fn environment_words() -> &'static [f32; FLOATS] {
    &environment().out
}

/// How far the wind has carried the clouds (`EnvironmentDraw::drift`).
pub(crate) fn environment_drift() -> [f64; 2] {
    environment().drift
}

/// The eye's altitude over the sea, in km, as the last frame saw it.
pub(crate) fn environment_alt_km() -> f64 {
    environment().alt_km
}

/// The cosines of the sun's and the moon's zenith angles at `days`, seen from
/// `[lon, lat]`. Each is the up component of the body's unit direction, so
/// how far the body stands does not matter.
fn mus(days: f64, [lon, lat]: [f64; 2]) -> [f64; 2] {
    let mu = |p: [f64; 3]| p[2] / v3::len(&p);
    [mu(sun_pos(days, lat, lon)), mu(moon_pos(days, lat, lon))]
}

/// The time, in Unix ms, the sky is drawn at, and whether that time is held.
/// `unix_ms` is the wall clock's time and `lon` the longitude in degrees.
/// With the `sky_time` setting at `real`, the sky follows the wall clock.
/// Otherwise it is held at the hour `time` names, in the place's mean solar
/// time today, so the sun stands where it does at that hour whatever the
/// clock reads.
fn sky_clock(time: SkyTime, unix_ms: f64, lon: f64) -> (f64, bool) {
    const HOUR_MS: f64 = 3_600_000.0;
    let hour = match time {
        SkyTime::Real => return (unix_ms, false),
        SkyTime::Dawn => 7.0,
        SkyTime::Morning => 9.0,
        SkyTime::Noon => 12.0,
        SkyTime::Afternoon => 15.0,
        SkyTime::Dusk => 17.0,
    };
    // The place's solar time runs ahead of UTC by its longitude, 15° an hour.
    let ahead = lon / 15.0 * HOUR_MS;
    let today = libm::floor((unix_ms + ahead) / (24.0 * HOUR_MS)) * 24.0 * HOUR_MS;
    (today + hour * HOUR_MS - ahead, true)
}

/// The step of the sun and of the moon, in the measure `mus` gives
/// (pxl-render `sky_steps`). For a light whose sky is drawn, its step
/// is how far its mu moves before the drawn sky changes by a display
/// level. For a light out of reach of the air, it is how far its mu moves
/// before the light comes into reach. A step is 0 until the renderer's first
/// measure of it lands, so until then every observation steps the sky.
fn steps() -> [f64; 2] {
    sky_steps().map(f64::from)
}

pub(crate) struct EnvironmentDraw {
    started: bool,
    /// The days since J2000.0 the sky is drawn for. They follow the sky's
    /// clock one step at a time (`observe`).
    days: f64,
    /// Where the sun and the moon are drawn. They ease toward `aimed`.
    sun: [f64; 3],
    moon: [f64; 3],
    /// Where the sun and the moon stand, and how the stars are turned, at
    /// `days` seen from `anchor`. Both are computed again whenever either of
    /// those changes (`aim`).
    aimed: [[f64; 3]; 2],
    star_frame: [[f64; 3]; 3],
    /// The place the sky is observed from. It follows the eye's anchor only
    /// once the sun or the moon would stand a step higher or lower from there
    /// (`steps`), so a pan too small to change the drawn sky moves no word of
    /// it. When it moves, `from` keeps where the sun and the moon were drawn,
    /// and `progress` runs from 0 to 1 as they ease from there to `aimed`.
    anchor: [f64; 2],
    from: [[f64; 3]; 2],
    progress: f64,
    stars: f64,
    /// How far the wind has carried the clouds, X and Y in units. It is taken
    /// at the clouds' clock, the wall time since the first observation, which
    /// holds still between its steps (`observe`).
    drift: [f64; 2],
    /// The eye's altitude over the sea, in km, as the last frame saw it
    /// (`tick`).
    alt_km: f64,
    /// The clocks of the sky and of the clouds, which step in Unix ms.
    sky_at: StepClock,
    clouds_at: StepClock,
    first_ms: Option<f64>,
    out: [f32; FLOATS],
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
            sky_at: StepClock::default(),
            clouds_at: StepClock::default(),
            first_ms: None,
            out: [0.0; FLOATS],
        }
    }
}

impl EnvironmentDraw {
    /// The place the sky should be seen from now. Before the sky starts it is
    /// the eye's anchor. After, it is the eye's anchor once the sun or the
    /// moon stands a step higher or lower from there than from the current
    /// place, and the current place otherwise.
    fn sky_anchor(&self) -> [f64; 2] {
        let [lon, lat, _] = anchor();
        let live = [lon, lat];
        if !self.started {
            return live;
        }
        let (was, is, step) = (mus(self.days, self.anchor), mus(self.days, live), steps());
        if (0..2).any(|k| (is[k] - was[k]).abs() >= step[k]) {
            live
        } else {
            self.anchor
        }
    }

    /// Whether the sun, as drawn, is under the horizon, where night begins to
    /// fall (pxl-content param.rs `night::FALL`).
    pub(crate) fn night(&self) -> bool {
        self.started && v3::norm(&self.sun)[2] < f64::from(night::FALL[1])
    }

    /// Observes the sky at the wall clock's time `unix_ms`, with the monotonic
    /// clock at `now_ms`. Returns how many milliseconds remain until the sky's
    /// next step, or, before the sky starts, the earlier of its and the
    /// clouds'; once it has started the clouds wake the frame at their own
    /// (`animation::read`). Each is a clock that holds still and steps a quantum at a
    /// time (`lens::StepClock`), so their words do not change between steps.
    /// A quantum is measured where its clock last stepped, at the rate things
    /// moved there. The sky's quantum is the time the sun or the moon takes
    /// to move its step (`steps`). The clouds' quantum is the time the wind
    /// takes to carry them the seen step at the nearest
    /// (`cloud::drift_step_s`). Before the sky starts, every observation
    /// steps both clocks. The sky's clock reads the hour the `sky_time`
    /// setting shows (`sky_clock`), and a held hour never steps it. The
    /// clouds' clock reads the wall clock.
    pub(crate) fn observe(&mut self, unix_ms: f64, now_ms: f64) -> f64 {
        let first = *self.first_ms.get_or_insert(unix_ms);
        let at = self.sky_anchor();
        let (sky_ms, fixed) = sky_clock(settings().sky_time, unix_ms, at[0]);
        let mus = |days: f64| mus(days, at);
        let step = steps();
        // The time at which each light's mu will have moved a step, at
        // the rate it moves over the minute after `at`. A light whose step
        // has not landed gives `unknown`. The sky's clock passes 0 for it, so
        // it steps at every observation. The wake passes infinity for it,
        // because the landing of the renderer's measures wakes a frame of its
        // own (pxl-render `complete`).
        const MINUTE_MS: f64 = 60_000.0;
        let rate = |at: f64, unknown: f64| {
            let days = j2000_days(at);
            let (now, then) = (mus(days), mus(days + MINUTE_MS / 86_400_000.0));
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
        // Once the sky has started, the clouds' drift is an animation. Before
        // it, every observation sets their clock anew, which is no
        // animation's step.
        let (clouds, clouds_in) = match self.started {
            true => {
                let (clouds, _) =
                    animation::read(&mut self.clouds_at, now_ms, unix_ms, 1.0, clouds_next);
                (clouds, f64::INFINITY)
            }
            false => {
                let (clouds, due, _) = self.clouds_at.at(unix_ms, clouds_next);
                (clouds, due - unix_ms)
            }
        };
        self.drift = drift((clouds - first) / 1000.0);
        let sky_in = if fixed {
            f64::INFINITY
        } else {
            rate(sky, f64::INFINITY) - sky_ms
        };
        sky_in.min(clouds_in).max(0.0)
    }

    fn aim(&mut self) {
        let [lon, lat] = self.anchor;
        self.aimed = [sun_pos(self.days, lat, lon), moon_pos(self.days, lat, lon)];
        self.star_frame = equatorial_to_local(self.days, lat, lon);
    }

    /// Advances the sky by one frame and writes its words. Returns whether a
    /// transition still wants frames: the sun and the moon easing across a
    /// jump of the anchor, or the stars fading in. The clock's steps are not
    /// eased. Each moves the drawn sky by a display level at most, so it is
    /// taken as it comes (`observe`), and a still sky's words do not change
    /// between steps.
    pub(crate) fn tick(&mut self, frame: &Frame) -> bool {
        if self.first_ms.is_none() {
            return false;
        }
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
        // The camera's Z, in units, is its altitude over the sea.
        let km_per_u = km_per_u();
        self.alt_km = frame.camera[2] * km_per_u;
        // The floor under the eye, as the frame observed it.
        let floor = observation::observation().map_or(0.0, |o| o.floor);
        let light = Light {
            sun: self.sun,
            moon: self.moon,
            stars: self.stars,
            alt_km: self.alt_km,
            floor_km: floor * km_per_u,
            km_per_u,
            star_frame: self.star_frame,
        };
        write_environment(&mut self.out, &light);
        self.progress < 1.0 || self.stars < 1.0
    }
}
