// pxl-kernel/layer/environment/theme_clock.rs

//! The one source of the time-of-day theme, checked once a minute so every
//! consumer reads the same value: asked on unrelated events, it went stale
//! for hours and consumers could disagree.

use pxl_spec::space::adapter::epsg4326::{self};

use crate::global::Global;
use crate::host::{local_time, now};
use crate::kernel::kernel;
use crate::layer::environment::theme::{get_theme_key, Theme};
use crate::world::change::wake_at;

const THEME_CHECK_INTERVAL_MS: f64 = 60_000.0;

// The theme is chosen for a real place before map_center exists.
use crate::control::navigation::bootstrap::FALLBACK_G4326;

fn compute_theme_key() -> Theme {
    let (lon, lat) = match crate::control::camera::controller::map_center() {
        Some(center) => {
            let g = epsg4326::unproject(center.i, center.f);
            (g[0], g[1])
        }
        None => (FALLBACK_G4326[0], FALLBACK_G4326[1]),
    };
    let t = local_time();
    let environment = &mut kernel().sched.environment;
    environment.daylight(lat, lon, t.day_start_ms);
    let [sunrise_hour, sunset_hour] = environment.daylight;
    get_theme_key(t.hours + t.minutes / 60.0 + t.seconds / 3600.0, sunrise_hour, sunset_hour)
}

struct ThemeClock {
    theme_key: Theme,
    checked_at: f64,
}

static CLOCK: Global<ThemeClock> = Global::new();

fn clock() -> &'static mut ThemeClock {
    CLOCK.get(|| ThemeClock {
        theme_key: compute_theme_key(),
        checked_at: now(),
    })
}

pub fn theme_clock_step() {
    let c = clock();
    if now() - c.checked_at < THEME_CHECK_INTERVAL_MS {
        wake_at(c.checked_at + THEME_CHECK_INTERVAL_MS);
        return;
    }
    c.checked_at = now();
    wake_at(c.checked_at + THEME_CHECK_INTERVAL_MS);
    let next = compute_theme_key();
    if next != c.theme_key {
        c.theme_key = next;
    }
}

pub fn theme_key() -> Theme {
    clock().theme_key
}
