// pxl_core/module/world/environment/theme_clock.rs

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

use pxl_shared::spec::space::adapter::epsg4326::{self, G4326};

use super::theme::{get_theme_key, WorldThemeKey};
use crate::global::{now, Global};
use crate::host::local_time;
use crate::kernel::kernel;
use crate::module::map::store::use_map_store;

const THEME_CHECK_INTERVAL_MS: f64 = 60_000.0;

// The same Seoul fallback the map bootstrap uses before a real GPS fix (or that
// fallback itself) resolves map_center.
const FALLBACK_LAT: f64 = 37.5665;
const FALLBACK_LON: f64 = 126.978;

fn compute_theme_key() -> WorldThemeKey {
    let (lon, lat) = match use_map_store().map_center {
        Some(center) => {
            let mut g: G4326 = [0.0; 3];
            epsg4326::unproject(&mut g, center.i, center.f);
            (g[0], g[1])
        }
        None => (FALLBACK_LON, FALLBACK_LAT),
    };
    let t = local_time();
    let environment = &mut kernel().sched.environment;
    environment.daylight(lat, lon, t.day_start_ms);
    let [sunrise_hour, sunset_hour] = environment.daylight;
    get_theme_key(t.hours + t.minutes / 60.0 + t.seconds / 3600.0, sunrise_hour, sunset_hour)
}

struct ThemeClock {
    theme_key: WorldThemeKey,
    checked_at: f64,
    revision: u32,
}

static CLOCK: Global<ThemeClock> = Global::new();

fn clock() -> &'static mut ThemeClock {
    CLOCK.get(|| ThemeClock {
        theme_key: compute_theme_key(),
        checked_at: now(),
        revision: 0,
    })
}

/// The theme clock's turn: a minute since the last check, check again.
pub fn theme_clock_step() {
    let c = clock();
    if now() - c.checked_at < THEME_CHECK_INTERVAL_MS {
        return;
    }
    c.checked_at = now();
    let next = compute_theme_key();
    if next != c.theme_key {
        c.theme_key = next;
        c.revision += 1;
    }
}

/// The current theme key.
pub fn use_theme_key() -> WorldThemeKey {
    clock().theme_key
}

/// Moves when the hour band changes.
pub fn theme_revision() -> u32 {
    clock().revision
}
