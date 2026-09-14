// @pxl/web/module/world/environment/theme_clock.ts

/**
 * Single shared source of truth for "what time-of-day theme is it right now".
 *
 * get_theme_key() in theme.ts is a pure function of the wall clock — calling
 * it directly inside a component only re-evaluates on that component's next
 * unrelated re-render, not when the clock actually crosses an hour boundary.
 * Four different components (environment.tsx, scene.tsx, location_puck.tsx,
 * html_portal.tsx) each called it independently, so the theme could go stale
 * for hours during idle viewing, and different components could show
 * different themes simultaneously if their last re-render happened to land
 * on opposite sides of an hour boundary.
 *
 * This ticks once a minute (hour-granularity theme, so that's plenty) and
 * every consumer reads the same store value, so they can't diverge.
 *
 * Also now location-aware: get_theme_key() needs the real sunrise/sunset
 * for wherever the map is centered (the module's environment,
 * crates/pxl-core/src/world/environment.rs, itself needing lat/lon), so each tick reads useMapStore's current map_center directly
 * via getState() rather than a React subscription — this module is a plain
 * singleton outside any component tree, and the interval only needs
 * whatever map_center happens to be *at tick time*, not to re-run the
 * instant it changes.
 */

import { create } from "zustand";

import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";

import { useMapStore } from "@/module/map/store.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import { get_theme_key, type WorldThemeKey } from "./theme.ts";

const THEME_CHECK_INTERVAL_MS = 60_000;

// Same Seoul fallback use_map_bootstrap.ts / environment.tsx already use
// before a real GPS fix (or that fallback itself) resolves map_center.
const FALLBACK_LAT = 37.5665;
const FALLBACK_LON = 126.978;
const latlon_scratch: G4326 = [0, 0, 0];

function current_hour(now: Date): number {
  return now.getHours() + now.getMinutes() / 60 + now.getSeconds() / 3600;
}

function compute_theme_key(): WorldThemeKey {
  const map_center = useMapStore.getState().map_center;
  const [lon, lat] = map_center ? epsg4326.unproject(latlon_scratch, map_center.i, map_center.f) : [FALLBACK_LON, FALLBACK_LAT];
  const now = new Date();
  const day_start = new Date(now);
  day_start.setHours(0, 0, 0, 0);
  let sunrise_hour = 6, sunset_hour = 18;
  if (CORE_PXL) {
    [sunrise_hour, sunset_hour] = new Float64Array(CORE_PXL.memory.buffer, CORE_PXL.pxl_env_daylight(lat, lon, day_start.getTime()), 2);
  }
  return get_theme_key(current_hour(now), sunrise_hour, sunset_hour);
}

type ThemeClockStore = {
  theme_key: WorldThemeKey;
};

export const useThemeClockStore = create<ThemeClockStore>(() => ({
  theme_key: compute_theme_key(),
}));

setInterval(() => {
  const next = compute_theme_key();
  if (useThemeClockStore.getState().theme_key !== next) {
    useThemeClockStore.setState({ theme_key: next });
  }
}, THEME_CHECK_INTERVAL_MS);

/** Reactive current theme key — re-renders the caller when the hour band changes. */
export function useThemeKey(): WorldThemeKey {
  return useThemeClockStore((state) => state.theme_key);
}
