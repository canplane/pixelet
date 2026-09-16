// pxl_core/module/map/environment.rs

//! The environment's inputs, handed to the scheduler's environment
//! (crates/pxl/core/src/world/environment.rs): the time of day's theme when it
//! changes, and each tick where the map is and what the wall clock says. The
//! environment places the sun and the moon, eases the light, the fog and the sky
//! toward the theme, and writes what the renderer draws them with.

use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use crate::global::Global;
use crate::host::wall_now;
use crate::kernel::kernel;
use crate::module::camera::config::camera_cfg;
use crate::module::map::store::use_map_store;
use crate::module::world::change::request_world_tick;
use crate::module::world::environment::config::{light_cfg, render_cfg};
use crate::module::world::environment::theme::{themes, WorldThemeKey};
use crate::module::world::environment::theme_clock::use_theme_key;
use crate::render::gpu::color::linear_rgb;
use crate::world::environment::THEME_FLOATS;

// Same Seoul coordinate use_map_bootstrap.rs seeds map_center with before a
// real GPS fix (or that fallback itself) resolves — reused here so the very
// first frame(s), before map_center is available, still get a real location
// instead of computing sun/moon position for (0,0).
const FALLBACK_LAT: f64 = 37.5665;
const FALLBACK_LON: f64 = 126.978;

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
        observed_lat: FALLBACK_LAT,
        observed_lon: FALLBACK_LON,
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
