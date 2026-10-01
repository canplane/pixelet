// pxl-kernel/layer/environment/theme_clock.rs

//! Single shared source of truth for "what time-of-day theme is it right now".
//!
//! get_theme_key() is a pure function of the wall clock — asking it only when
//! something else happens re-evaluates it on that unrelated event, not when the
//! clock actually crosses an hour boundary. Several consumers each asked it
//! independently, so the theme could go stale for hours during idle viewing,
//! and different consumers could show different themes at once if their last
//! ask happened to land on opposite sides of an hour boundary.
//!
//! This checks once a minute (hour-granularity theme, so that's plenty) and
//! every consumer reads the same value, so they can't diverge. Location-aware:
//! the real sunrise/sunset for wherever the map is centered comes from the
//! kernel's environment, which needs lat/lon.

use pxl_spec::space::adapter::epsg4326::{self, G4326};

use crate::global::Global;
use crate::host::{local_time, now};
use crate::kernel::kernel;
use crate::layer::environment::theme::{get_theme_key, Theme};
use crate::world::change::wake_at;

const THEME_CHECK_INTERVAL_MS: f64 = 60_000.0;

// The map's own fallback, so a theme is chosen for a real place before
// map_center exists.
use crate::control::navigation::bootstrap::FALLBACK_G4326;

fn compute_theme_key() -> Theme {
    let (lon, lat) = match crate::control::camera::controller::map_center() {
        Some(center) => {
            let mut g: G4326 = [0.0; 3];
            epsg4326::unproject(&mut g, center.i, center.f);
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

/// The theme clock's turn: a minute since the last check, check again.
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

/// The current theme key.
pub fn theme_key() -> Theme {
    clock().theme_key
}
